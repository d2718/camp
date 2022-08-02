/*!
Interoperation between the client (user) and server.

(Not the application and the database; that's covered by `auth` and `store`.)
*/
use std::{
    fmt::Debug,
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
use tokio::sync::RwLock;

static TEMPLATES: OnceCell<Handlebars> = OnceCell::new();

static ERROR_500: &str = r#"<!doctype html>
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

pub fn respond_500() -> Response<String> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "text/html")
        .header(header::CONTENT_LENGTH, ERROR_500.len())
        .header(header::CACHE_CONTROL, "no-store")
        .body(ERROR_500.to_owned()).unwrap()
}

pub fn string_response(
    code: StatusCode,
    content_type: &str,
    body: String,
    addl_headers: &[(HeaderName, &[u8])],
) -> Response<String> {
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
                return respond_500()
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
            respond_500()
        }
    }
}

pub fn serve_template<S>(
    code: StatusCode,
    template_name: &str,
    data: &S,
    addl_headers: &[(HeaderName, &[u8])]
) -> Response<String>
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
            respond_500()
        },
    }
}