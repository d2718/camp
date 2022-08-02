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

pub fn respond_500() -> Response<Full<_>> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "text/html")
        .header(header::CONTENT_LENGTH, ERROR_500.len())
        .header(header::CACHE_CONTROL, "no-store")
        .body(Full::from(ERROR_500.)).unwrap()
}

pub fn serve_template<S>(
    code: StatusCode,
    template: &str,
    data: &S
) -> Response<Full<_>> 
where
    S: Serialize + Debug
{
    log::trace!("serve_template( {}, {:?}, ... ) called.", &code, template);

    match TEMPLATES.get().unwrap().render_template(template, data) {
        Ok(response_body) => Response::builder()
            .status(code)
            .header(header::CONTENT_TYPE, "text/html")
            .header(header::CONTENT_LENGTH, response_body.len())
            .header(header::CACHE_CONTROL, "no-store")
            .body(Full::from(response_body)).unwrap(),
        Err(e) => {
            log::error!("Error rendering template {:?} with data {:?}", template, data);
            respond_500()
        },
    }
}