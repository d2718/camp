/*!
Here we go!
*/
use axum::{
    //error_handling::HandleErrorLayer,
    //Extension,
    Form,
    http::{header, StatusCode},
    response::{ErrorResponse, Html, IntoResponse, Response},
    Router,
    routing::{get_service, post},
};
use serde::Deserialize;
use serde_json::json;
use simplelog::{ColorChoice, TerminalMode, TermLogger};
use tower_http::{
    services::fs::{ServeDir, ServeFile},
};

use camp::{
    auth, auth::AuthResult,
    config, config::Glob,
    course::{Course, Chapter},
    inter,
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

async fn handle_login(
    Form(_form): Form<LoginData>
) -> Response<String> {
    let data = json!({
        "error_message": "You attempted to log in, but logging in is currently unimplemented."
    });

    inter::serve_template(
        StatusCode::NOT_IMPLEMENTED,
        "login_error",
        &data,
        &[
            (header::SERVER, b"axum"),
            (header::HeaderName::from_static("camp-special"), b"tomato paste"),
        ]
    )
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
        .route("/login", post(handle_login));
    
    axum::Server::bind(&glob.addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
