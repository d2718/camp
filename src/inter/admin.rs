/*!
Subcrate for interoperation with Admin users.
*/
use std::sync::Arc;

use axum::{
    extract::Extension,
    http::header::{HeaderMap, HeaderName},
};
use tokio::sync::RwLock;

use crate::config::Glob;
use crate::{auth, auth::AuthResult, user::*};
use super::*;

pub async fn login(
    base: BaseUser,
    form: LoginData,
    glob: Arc<RwLock<Glob>>
) -> Response {
    log::trace!(
        "admin::login( {:?}, {:?}, [ global state ] ) called.",
        &base, &form
    );

    let auth_db = {
        let glob = glob.read().await;
        auth::Db::new(glob.auth_db_connect_string.clone())
    };

    let auth_key = match auth_db.check_password_and_issue_key(
        &base.uname,
        &form.password,
        &base.salt
    ).await {
        Err(e) => {
            log::error!(
                "Error: auth::Db::check_password_and_issue_key( {:?}, {:?}, [ Glob ]): {}",
                &base, &form, &e
            );
            return html_500();
        },
        Ok(AuthResult::Key(k)) => k,
        Ok(AuthResult::BadPassword) => {
            return respond_bad_password();
        },
        Ok(x) => {
            log::warn!(
                "auth::Db::check_password_and_issue_key( {:?}, {:?}, [ Glob ] ) returned {:?}, which shouldn't happen.",
                &base, &form, &x
            );
            return respond_bad_password();
        }
    };

    let key_header_value = match HeaderValue::try_from(auth_key) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Auth key unable to be converted to HTTP header value: {}", &e);
            return html_500();
        }
    };

    serve_static(
        StatusCode::OK,
        "static/admin.html",
        vec![(
                HeaderName::from_static("x-camp-key"),
                key_header_value
            )]
    )
}

/* pub async fn api(
    headers: HeaderMap,
    Extension(glob): Extension<Arc<RwLock<Glob>>>
) -> CampResponse {
    let u = match key_authenticate(&headers, glob.clone()).await {
        Ok(u) => u,
        Err(resp) => { return resp; },
    };

    let admin = match u {
        User::Admin(a) => a,
        _ => {
            return string_response(
                StatusCode::FORBIDDEN,
                "text/plain",
                "Who is this? What's your operating number?".to_owned(),
                &[]
            );
        },
    };

    string_response(
        StatusCode::NOT_IMPLEMENTED,
        "text/plain",
        "Sorry, this isn't implemented yet.".to_owned(),
        &[]
    )
} */