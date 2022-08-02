/*!
Interoperation between the client (user) and server.

(Not the application and the database; that's covered by `auth` and `store`.)
*/
use std::{
    fmt::{Debug, Display},
    path::{Path, PathBuf},
};

use axum::{
    body::{Bytes, Full},
    http::{header, StatusCode},
    http::header::{HeaderName, HeaderValue},
    response::Response,
};
use handlebars::Handlebars;
use once_cell::sync::OnceCell;
use serde::Serialize;
use serde_json::json;
use tokio::sync::RwLock;

pub mod admin;

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

pub type CampResponse = Response<String>;

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
    let template_dir = template_dir.as_ref();

    let mut h = Handlebars::new();
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
pub fn html_500() -> CampResponse {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "text/html")
        .header(header::CONTENT_LENGTH, HTML_500.len())
        .header(header::CACHE_CONTROL, "no-store")
        .body(HTML_500.to_owned()).unwrap()
}

/**
Return plain text response in the case of an unrecoverable* error.

(*"Unrecoverable" from the perspective of fielding the current request,
not from the perspective of the program crashing.)
*/
pub fn text_500(text: Option<String>) -> CampResponse {
    let r = Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_TYPE, "text/plain");
    match text {
        Some(text) => r.header(header::CONTENT_LENGTH, text.len())
            .body(text).unwrap(),
        None => r.header(header::CONTENT_LENGTH, TEXT_500.len())
            .body(TEXT_500.to_owned()).unwrap()
    }
}

pub fn string_response(
    code: StatusCode,
    content_type: &str,
    body: String,
    addl_headers: &[(HeaderName, &[u8])],
) -> CampResponse {
    log::trace!(
        "string_response( {}, {}, [ {} bytes of body ], [ {} add'l headers ] ) called.",
        &code, content_type, body.len(), addl_headers.len()
    );
    let content_length = body.len();
    let mut r = Response::builder()
        .status(code)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, content_length)
        .header(header::CACHE_CONTROL, "no-store");
    for (name, value) in addl_headers.iter() {
        match HeaderValue::from_bytes(value) {
            Ok(v) => { r = r.header(name, v); },
            Err(e) => {
                log::error!(
                    "Error converting \"{}\" into header value: {}",
                    &String::from_utf8_lossy(value), &e
                );
                if content_type == "text/html" {
                    return html_500();
                } else {
                    return text_500(None);
                }
            }
        }
    }
    match r.body(body) {
        Ok(r) => r,
        Err(e) => {
            log::error!(
                "Error generating string_response( {:?}, {:?}, {} body bytes):\n{}",
                code, content_type, content_length, &e
            );
            if content_type == "text/html" {
                html_500()
            } else {
                text_500(None)
            }
        }
    }
}

pub fn serve_template<S>(
    code: StatusCode,
    template_name: &str,
    data: &S,
    addl_headers: &[(HeaderName, &[u8])]
) -> CampResponse
where
    S: Serialize + Debug
{
    log::trace!("serve_template( {}, {:?}, ... ) called.", &code, template_name);

    match TEMPLATES.get().unwrap().render(template_name, data) {
        Ok(response_body) => string_response(
            code,
            "text/html",
            response_body,
            addl_headers
        ),
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
    addl_headers: &[(HeaderName, &[u8])]
) -> CampResponse {
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

    string_response(
        code,
        "text/html",
        body,
        addl_headers,
    )
}

pub fn respond_bad_password() -> CampResponse {
    log::trace!("respond_bad_password() called.");

    let data = json!({
        "error_message": "Invalid username/password combination."
    });

    serve_template(
        StatusCode::UNAUTHORIZED,
        "login_error",
        &data,
        &[]
    )
}