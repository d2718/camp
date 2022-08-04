/*!
Subcrate for interoperation with Admin users.
*/
use std::sync::Arc;

use axum::{
    extract::Extension,
    http::header::{HeaderMap, HeaderName},
};
use serde_json::json;
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

    let auth_response = {
        glob.read().await.auth().read().await.check_password_and_issue_key(
            &base.uname,
            &form.password,
            &base.salt
        ).await
    };

    let auth_key = match auth_response {
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

    let data = json!({
        "uname": &base.uname,
        "key": &auth_key
    });

    serve_template(
        StatusCode::OK,
        "admin",
        &data,
        vec![]
    )
}

pub async fn api(
    headers: HeaderMap,
    Extension(glob): Extension<Arc<RwLock<Glob>>>
) -> Response {

    let uname: &str = match headers.get("x-camp-uname") {
        Some(uname) => match uname.to_str() {
            Ok(s) => s,
            Err(_) => { return text_500(None); }
        },
        None => { return text_500(None); },
    };

    let u = {
        let glob = glob.read().await;
        if let Some(u) = glob.users.get(uname) {
            u.clone()
        } else {
            return text_500(None);
        }
    };

    let admin = match u {
        User::Admin(a) => a,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                "Who is this? What's your operating number?".to_owned(),
            ).into_response();
        },
    };

    (
        StatusCode::NOT_IMPLEMENTED,
        format!("Sorry, {}, this isn't implemented yet.", &admin.uname)
    ).into_response()
}