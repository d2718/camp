/*!
Here we go!
*/
use std::sync::Arc;

use axum::{
    //error_handling::HandleErrorLayer,
    Extension,
    Form,
    http::StatusCode,
    response::{IntoResponse, Response},
    Router,
    routing::{get_service, post},
};
use serde_json::json;
use simplelog::{ColorChoice, TerminalMode, TermLogger};
use tokio::sync::RwLock;
use tower_http::{
    services::fs::{ServeDir, ServeFile},
};

use camp::{
    config, config::Glob,
    inter,
    user::User,
};

async fn catchall_error_handler(e: std::io::Error) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unhandled internal error: {}", &e)
    )
}



async fn handle_login(
    Form(form): Form<inter::LoginData>,
    Extension(glob): Extension<Arc<RwLock<Glob>>>
) -> Response {
    log::trace!("handle_login( {:?}, [ global state ]) called.", &form);

    let user = {
        let glob = glob.read().await;
        match glob.users.get(&form.uname) {
            Some(u) => u.clone(),
            None => { return inter::respond_bad_password(); }
        }
    };

    if let User::Admin(a) = user {
        return inter::admin::login(a, form, glob.clone()).await;
    }

    let data = json!({
        "error_message": "You attempted to log in, but logging in is currently unimplemented."
    });

    inter::serve_template(
        StatusCode::NOT_IMPLEMENTED,
        "login_error",
        &data,
        vec![]
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
    let glob = Arc::new(RwLock::new(glob));

    let serve_root = get_service(ServeFile::new("data/index.html"))
        .handle_error(catchall_error_handler);

    let serve_static = get_service(ServeDir::new("static"))
        .handle_error(catchall_error_handler);

    let addr = glob.read().await.addr.clone();
    let app = Router::new()
        .route("/", serve_root)
        .nest("/static", serve_static)
        .route("/login", post(handle_login))
            .layer(Extension(glob));
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
