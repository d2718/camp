/*!
Here we go!
*/
use std::{
    //collections::HashMap,
    fmt::Write,
};

use axum::{
    //error_handling::HandleErrorLayer,
    //Extension,
    Form,
    http::StatusCode, // header
    response::{ErrorResponse, Html, IntoResponse},
    Router,
    routing::{get_service, post},
};
use serde::Deserialize;
use simplelog::{ColorChoice, TerminalMode, TermLogger};
use tower_http::{
    services::fs::{ServeDir, ServeFile},
};

use camp::{
    auth, auth::AuthResult,
    config, config::Glob,
    course::{Course, Chapter},
    store::Store,
    user::{BaseUser, Role, Student, Teacher, User},
};

async fn catchall_error_handler(e: std::io::Error) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unhandled internal error: {}", &e)
    )
}

#[derive(Deserialize)]
struct LoginData {
    uname: String,
    password: String,
}

async fn dummy_login(
    Form(form): Form<LoginData>
) -> Result<Html<String>, ErrorResponse> {
    let mut buff = std::fs::read_to_string("data/dummy_login_head.html")
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error generating response: {}", &e)
        ))?;
    
    write!(
        &mut buff,
        "
<tr><td>uname:</td><td>{}</td></tr>
<tr><td>password:</td><td>{}</td></tr>
        ",
        &form.uname, &form.password
    ).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error generating response: {}", &e)
    ))?;

    buff.push_str(
        &std::fs::read_to_string("data/dummy_login_foot.html")
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error generating response: {}", &e)
            ))?
    );

    Ok(Html::from(buff))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let log_cfg = simplelog::ConfigBuilder::new()
        .add_filter_allow_str("camp")
        .build();
    TermLogger::init(
        camp::log_level_from_env(),
        log_cfg,
        TerminalMode::Stdout,
        ColorChoice::Auto
    ).unwrap();
    log::info!("Logging started.");

    let glob = config::load_configuration("config.toml").await.unwrap();
    log::info!("Global variables:\n{:#?}", &glob);

    let serve_root = get_service(ServeFile::new("data/index.html"))
        .handle_error(catchall_error_handler);

    let serve_static = get_service(ServeDir::new("static"))
        .handle_error(catchall_error_handler);

    let app = Router::new()
        .route("/", serve_root)
        .nest("/static", serve_static)
        .route("/login", post(dummy_login));
    
    axum::Server::bind(&glob.addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
