/*!
Interoperation between the client (user) and server.

(Not the application and the database; that's covered by `auth` and `store`.)
*/
use std::{
    fmt::{Debug, Display},
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    body::{Bytes, Full},
    http::{header, Request, StatusCode},
    http::header::{HeaderMap, HeaderName, HeaderValue},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
};
use handlebars::Handlebars;
use once_cell::sync::OnceCell;
use serde::Serialize;
use serde_json::json;
use tokio::sync::RwLock;

use crate::auth::AuthResult;
use crate::config::Glob;

pub mod admin;
pub mod teacher;

static TEMPLATES: OnceCell<Handlebars> = OnceCell::new();

static HTML_500: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>camp | Error</title>
<link rel="stylesheet" href="/static/camp.css">
</head>
<body>
<h1>Internal Server Error</h1>
<p>(Error 500)</p>
<p>Something went wrong on our end. No further or more
helpful information is available about the problem.</p>
</body>
</html>"#;

static TEXT_500: &str = "An internal error occurred; an appropriate response was inconstructable.";

trait AddHeaders: IntoResponse + Sized {
    fn add_headers(self, mut new_headers: Vec<(HeaderName, HeaderValue)>) -> Response {
        let mut r = self.into_response();
        let r_headers = r.headers_mut();
        for (name, value) in new_headers.drain(..) {
            r_headers.insert(name, value);
        }

        r
    }
}

impl<T: IntoResponse + Sized> AddHeaders for T {}

/// Data type to read the form data from a front-page login request.
#[derive(serde::Deserialize, Debug)]
pub struct LoginData {
    pub uname: String,
    pub password: String,
}

/**
Initializes the resources used in this module. This function should be called
before any functionality of this module or any of its submodules is used.

Currently the only thing that happens here is loading the templates used by
`serve_template()`, which will panic unless `init()` has been called first.

The argument is the path to the directory where the templates used by
`serve_template()` can be found.
*/
pub fn init<P: AsRef<Path>>(template_dir: P) -> Result<(), String> {
    if TEMPLATES.get().is_some() {
        log::warn!("Templates directory already initialized; ignoring.");
        return Ok(())
    }

    let template_dir = template_dir.as_ref();

    let mut h = Handlebars::new();
    #[cfg(debug_assertions)]
    h.set_dev_mode(true);
    h.register_templates_directory(".html", template_dir)
        .map_err(|e| format!(
            "Error registering templates directory {}: {}",
            template_dir.display(), &e
        ))?;

    TEMPLATES.set(h)
        .map_err(|old_h| {
            let mut estr = String::from("Templates directory already registered w/templates:");
            for template_name in old_h.get_templates().keys() {
                estr.push('\n');
                estr.push_str(template_name.as_str());
            }
            estr
        })?;

    Ok(())
}

/**
Return an HTML response in the case of an unrecoverable* error.

(*"Unrecoverable" from the perspective of fielding the current request,
not from the perspective of the program crashing.)
*/
pub fn html_500() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(HTML_500)
    ).into_response()
}

pub fn text_500(text: Option<String>) -> Response {
    match text {
        Some(text) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            text
        ).into_response(),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            TEXT_500.to_owned()
        ).into_response()
    }
}

pub fn serve_template<S>(
    code: StatusCode,
    template_name: &str,
    data: &S,
    addl_headers: Vec<(HeaderName, HeaderValue)>
) -> Response
where
    S: Serialize + Debug
{
    log::trace!("serve_template( {}, {:?}, ... ) called.", &code, template_name);

    match TEMPLATES.get().unwrap().render(template_name, data) {
        Ok(response_body) => (
            code,
            Html(response_body)
        ).add_headers(addl_headers),
        Err(e) => {
            log::error!(
                "Error rendering template {:?} with data {:?}:\n{}",
                template_name, data, &e
            );
            html_500()
        },
    }
}

pub fn serve_static<P: AsRef<std::path::Path>>(
    code: StatusCode,
    path: P,
    addl_headers: Vec<(HeaderName, HeaderValue)>,
) -> Response {
    let path = path.as_ref();
    log::trace!("serve_static( {:?}, {}, [ {} add'l headers ] ) called.",
        &code, path.display(), addl_headers.len()
    );

    let body = match std::fs::read_to_string(path) {
        Ok(body) => body,
        Err(e) => {
            log::error!("Error attempting to serve file {}: {}", path.display(), &e);
            return html_500();
        }
    };

    (
        code,
        Html(body)
    ).add_headers(addl_headers)
}

pub fn respond_bad_password() -> Response {
    log::trace!("respond_bad_password() called.");

    let data = json!({
        "error_message": "Invalid username/password combination."
    });

    serve_template(
        StatusCode::UNAUTHORIZED,
        "login_error",
        &data,
        vec![]
    )
}

pub fn respond_bad_key() -> Response {
    log::trace!("respond_bad_key() called.");

    (
        StatusCode::UNAUTHORIZED,
        "Invalid authorization key.".to_owned(),
    ).into_response()
}

pub fn respond_bad_request(msg: String) -> Response {
    log::trace!("respond_bad_request( {:?} ) called.", &msg);

    (
        StatusCode::BAD_REQUEST,
        msg
    ).into_response()
}

/// Middleware function to ensure `x-camp-request-id` header is
/// maintained between request and response.
pub async fn request_identity<B>(
    req: Request<B>,
    next: Next<B>
) -> Response {
    let id_header = match req.headers().get("x-camp-request-id") {
        Some(id) => id.to_owned(),
        None => {
            return respond_bad_request(
                "Request must have an x-camp-request-id header.".to_owned()
            );
        },
    };

    let mut response = next.run(req).await;
    response.headers_mut().insert("x-camp-request-id", id_header);
    response
}

pub async fn key_authenticate<B>(
    req: Request<B>,
    next: Next<B>,
) -> Response {
    let glob: &Arc<RwLock<Glob>> = req.extensions().get().unwrap();

    let key = match req.headers().get("x-camp-key") {
        Some(k_val) => match k_val.to_str() {
            Ok(s) => s,
            Err(e) => {
                log::error!(
                    "Failed converting auth key value {:?} to &str: {}",
                    k_val, &e
                );
                return respond_bad_request(
                    "x-camp-key value unrecognizable.".to_owned()
                );
            },
        },
        None => {
            return respond_bad_request(
                "Request must have an x-camp-key header.".to_owned()
            );
        },
    };

    let uname = match req.headers().get("x-camp-uname") {
        Some(u_val) => match u_val.to_str() {
            Ok(s) => s,
            Err(e) => {
                log::error!(
                    "Failed converting uname value {:?} to &str: {}",
                    u_val, &e
                );
                return respond_bad_request(
                    "x-camp-uname value unrecognizable.".to_owned()
                );
            },
        },
        None => {
            return respond_bad_request(
                "Request must have an x-camp-uname header.".to_owned()
            );
        },
    };

    // Lololol the chain here.
    //
    // But seriously, we return the result, then match on the returned value,
    // instead of just matching on the huge-ass chain expression so that
    // the locks will release.
    let res = glob.read().await.auth().read().await.check_key(
        uname, key
    ).await;

    match res {
        Err(e) => {
            log::error!(
                "auth::Db::check_key( {:?}, {:?} ) returned error: {}",
                uname, key, &e
            );

            return text_500(None);
        },
        Ok(AuthResult::InvalidKey) => {
            return (
                StatusCode::UNAUTHORIZED,
                "Invalid authorization key.".to_owned(),
            ).into_response();
        },
        Ok(AuthResult::Ok) => {
            // This is the good path. We will just fall through and call the
            // next layer after the match.
        }
        Ok(x) => {
            log::warn!(
                "auth::Db::check_key() returned {:?}, which should never happen.",
                &x
            );
            return text_500(None);
        },
    }

    next.run(req).await
}