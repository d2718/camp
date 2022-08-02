/*!
Subcrate for interoperation with Admin users.
*/
use std::sync::Arc;

use axum::http::header::HeaderName;
use tokio::sync::RwLock;

use crate::config::Glob;
use crate::{auth, auth::AuthResult, user::*};
use super::*;

pub async fn login(
    base: BaseUser,
    form: LoginData,
    glob: Arc<RwLock<Glob>>
) -> CampResponse {
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

    serve_static(
        StatusCode::OK,
        "static/admin.html",
        &[(
            HeaderName::from_static("x-camp-key"), auth_key.as_bytes()
        )]
    )
}