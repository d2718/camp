/*!
Subcrate for interoperation with Admin users.
*/
use std::sync::Arc;

use axum::{
    extract::Extension,
    http::header::{HeaderMap, HeaderName},
    Json,
    response::{IntoResponse, Response},
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
    body: Option<String>,
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

    match u {
        User::Admin(_) => { /* Okay, request may proceed. */ },
        _ => {
            return (
                StatusCode::FORBIDDEN,
                "Who is this? What's your operating number?".to_owned(),
            ).into_response();
        },
    };

    let action = match headers.get("x-camp-action") {
        Some(act) => match act.to_str() {
            Ok(s) => s,
            Err(_) => { return respond_bad_request(
                "x-camp-action header unrecognizable.".to_owned()
            ); },
        },
        None => {
            return respond_bad_request(
                "Request must have an x-camp-action header.".to_owned()
            );
        },
    };

    match action {
        "populate-admins" => {
            populate_admins(glob.clone()).await
        },
        x => {
            respond_bad_request(
                format!("{:?} is not a recognizable x-camp-action value.", x)
            )
        },
    }
}

async fn populate_admins(glob: Arc<RwLock<Glob>>) -> Response {
    log::trace!("populate_admins( Glob ) called.");

    let glob = glob.read().await;
    let admins: Vec<&User> = glob.users.iter()
        .map(|(_, u)| u)
        .filter(|&u| u.role() == Role::Admin)
        .collect();

    (
        StatusCode::OK,
        [(
            HeaderName::from_static("x-camp-action"),
            HeaderValue::from_static("populate-admins")
        )],
        Json(admins),
    ).into_response()
}

async fn add_user(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let body = match body {
        Some(body) => body,
        None => { return respond_bad_request(
            "Request requires a JSON body.".to_owned()
        ); },
    };

    let u: User = match serde_json::from_str(&body) {
        Ok(u) => u,
        Err(e) => {
            log::error!(
                "Error deserializing JSON {:?} as BaseUser: {}",
                &body, &e
            );
            return text_500(Some("Unable to deserialize User struct.".to_owned()));
        },
    };

    {
        let mut glob = glob.write().await;
        if let Err(e) = glob.insert_user(&u).await {
            log::error!(
                "Error inserting new user ({:?})into database: {}",
                &u,&e,
            );
            return text_500(Some("Unable to insert User into database.".to_owned()));
        }
        if let Err(e) = glob.refresh_users().await {
            log::error!(
                "Error refreshing user hash from database: {}" ,&e
            );
            return text_500(Some("Unable to reread users from database.".to_owned()));
        }
    }

    populate_admins(glob).await
}