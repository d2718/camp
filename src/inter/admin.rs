/*!
Subcrate for interoperation with Admin users.
*/
use std::io::Cursor;
use std::sync::Arc;

use axum::{
    extract::Extension,
    http::header::{HeaderMap, HeaderName},
    Json,
    response::{IntoResponse, Response},
};
use serde_json::json;
use time::Date;
use tokio::sync::RwLock;

use crate::config::Glob;
use crate::course::{Course, Chapter};
use crate::{auth, auth::AuthResult, DATE_FMT, user::*};
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
        "populate-users" => populate_users(glob.clone()).await,
        "populate-admins" => populate_role(glob.clone(), Role::Admin).await,
        "populate-bosses" => populate_role(glob.clone(), Role::Boss).await,
        "add-user" => add_user(body, glob.clone()).await,
        "update-user" => update_user(body, glob.clone()).await,
        "delete-user" => delete_user(body, glob.clone()).await,
        "upload-students" => upload_students(body, glob.clone()).await,
        "populate-courses" => populate_courses(glob.clone()).await,
        "upload-course" => upload_course(body, glob.clone()).await,
        "add-course" => add_course(body, glob.clone()).await,
        "delete-course" => delete_course(body, glob.clone()).await,
        "update-course" => update_course(body, glob.clone()).await,
        "add-chapters" => add_chapters(body, glob.clone()).await,
        "update-chapter" => update_chapter(body, glob.clone()).await,
        "delete-chapter" => delete_chapter(body, glob.clone()).await,
        "populate-cal" => populate_calendar(glob.clone()).await,
        "update-cal" => update_calendar(body, glob.clone()).await,
        x => respond_bad_request(
            format!("{:?} is not a recognizable x-camp-action value.", x)
        ),
    }
}

async fn populate_role(glob: Arc<RwLock<Glob>>, role: Role) -> Response {
    log::trace!("populate_role( Glob, {:?} ) called.", &role);

    let glob = glob.read().await;
    let users: Vec<&User> = glob.users.iter()
        .map(|(_, u)| u)
        .filter(|&u| u.role() == role)
        .collect();

    (
        StatusCode::OK,
        [(
            HeaderName::from_static("x-camp-action"),
            HeaderValue::from_static("populate-users")
        )],
        Json(users),
    ).into_response()
}

async fn populate_users(glob: Arc<RwLock<Glob>>) -> Response {
    log::trace!("populate_all( Glob ) called.");

    let glob = glob.read().await;
    let mut users: Vec<&User> = glob.users.iter()
        .map(|(_, u)| u)
        .collect();
    users.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    (
        StatusCode::OK,
        [(
            HeaderName::from_static("x-camp-action"),
            HeaderValue::from_static("populate-users")
        )],
        Json(users),
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
            return text_500(Some(
                format!("Unable to insert User into database: {}", &e)
            ));
        }
        if let Err(e) = glob.refresh_users().await {
            log::error!(
                "Error refreshing user hash from database: {}" ,&e
            );
            return text_500(Some("Unable to reread users from database.".to_owned()));
        }
    }

    //populate_role(glob, u.role()).await
    populate_users(glob).await
}

async fn upload_students(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let body = match body {
        Some(body) => body,
        None => { return respond_bad_request(
            "Request requires a CSV body.".to_owned()
        ); }
    };

    {
        let glob = glob.read().await;
        if let Err(e) = glob.upload_students(&body).await {
            log::error!(
                "Error uploading new students via CSV: {}\n\nCSV text:\n\n{}\n",
                &e, &body
            );
            return text_500(Some(e.to_string()));
        }
    }
    {
        let mut glob = glob.write().await;
        if let Err(e) = glob.refresh_users().await {
            log::error!("Error refreshing user hash from database: {}", &e);
            return text_500(Some("Unable to reread users from database.".to_owned()));
        }
    }

    populate_users(glob).await
}

async fn update_user(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
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
                "Error deserializing JSON {:?} as User: {}",
                &body, &e
            );
            return text_500(Some("Unable to deserialize User struct.".to_owned()));
        },
    };

    {
        let mut glob = glob.write().await;
        if let Err(e) = glob.update_user(&u).await {
            log::error!(
                "Error updating user {:?}: {}", &u, &e,
            );
            return text_500(Some(e.to_string()));
        }
        if let Err(e) = glob.refresh_users().await {
            log::error!(
                "Error refreshing user hash from database: {}" ,&e
            );
            return text_500(Some("Unable to reread users from database.".to_owned()));
        }
    }

    //populate_role(glob, u.role()).await
    populate_users(glob).await
}

async fn delete_user(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let uname = match body {
        Some(uname) => uname,
        None => { return respond_bad_request(
            "Request must include the uname to delete as a body.".to_owned()
        ); },
    };

    {
        let glob = glob.read().await;
        if let Err(e) = glob.delete_user(&uname).await {
            log::error!(
                "Error deleting user {:?}: {}", uname, &e
            );
            return text_500(Some(e.to_string()));
        }
    }
    {
        if let Err(e) = glob.write().await.refresh_users().await {
            log::error!(
                "Error refreshing user hash from database: {}", &e
            );
            return text_500(Some("Unable to reread users from database.".to_owned()));
        }
    }

    populate_users(glob).await
}

//
//
// This section is for dealing with COURSES.
//
//

async fn populate_courses(glob: Arc<RwLock<Glob>>) -> Response {
    let glob = glob.read().await;

    let mut courses: Vec<&Course> = glob.courses.iter()
        .map(|(_, c)| c)
        .collect();
    
    courses.sort_by(|a, b|
        a.level.partial_cmp(&b.level).unwrap_or(std::cmp::Ordering::Equal)
    );

    (
        StatusCode::OK,
        [(
            HeaderName::from_static("x-camp-action"),
            HeaderValue::from_static("populate-courses")
        )],
        Json(courses)
    ).into_response()
}

async fn refresh_and_repopulate_courses(glob: Arc<RwLock<Glob>>) -> Response {
    {
        let mut glob = glob.write().await;
        if let Err(e) = glob.refresh_courses().await {
            log::error!(
                "Error refreshing course hash from database: {}", &e
            );
            return text_500(Some(
                format!("Unable to refresh course data from database: {}", &e)
            ));
        }
    }

    populate_courses(glob).await
}

async fn populate_calendar(glob: Arc<RwLock<Glob>>) -> Response {
    let date_strs: Vec<String> = glob.read().await.calendar.iter()
        .map(|d| format!("{}", d))
        .collect();
    
    (
        StatusCode::OK,
        [(
            HeaderName::from_static("x-camp-action"),
            HeaderValue::from_static("populate-cal")
        )],
        Json(date_strs)
    ).into_response()
}

async fn refresh_and_repopulate_calendar(glob: Arc<RwLock<Glob>>) -> Response {
    {
        let mut glob = glob.write().await;
        if let Err(e) = glob.refresh_calendar().await {
            log::error!(
                "Error refreshing calendar Vec from database: {}", &e
            );
            return text_500(Some(format!(
                "Unable to refresh calendar data from database: {}", &e
            )));
        }
    }

    populate_calendar(glob).await
}

async fn upload_course(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let body = match body {
        Some(body) => body,
        None => { return respond_bad_request(
            "Request requires textual body.".to_owned()
        ); },
    };

    let reader = Cursor::new(body);
    let crs = match Course::from_reader(reader) {
        Ok(crs) => crs,
        Err(e) => { return respond_bad_request(e); },
    };

    {
        let glob = glob.read().await;
        let data = glob.data();
        match data.read().await.insert_courses(&vec![crs]).await {
            Ok((n_crs, n_ch)) => {
                log::trace!(
                    "Inserted {} Cours(es) and {} Chapter(s) into the Data DB.",
                    n_crs, n_ch
                );
            },
            Err(e) => {
                return text_500(Some(e.into()));
            },
        };
    }

    refresh_and_repopulate_courses(glob).await
}

async fn add_course(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let body = match body {
        Some(body) => body,
        None => { return respond_bad_request(
            "Request requires application/json body describing the Course.".to_owned()
        ); },
    };

    let crs: Course = match serde_json::from_str(&body) {
        Ok(crs) => crs,
        Err(e) => {
            log::error!(
                "Error deserializing JSON {:?} as Course: {}",
                &body, &e
            );
            return text_500(Some("Unable to deserialize to Course struct.".to_owned()));
        },
    };

    {
        let glob = glob.read().await;
        let data = glob.data();
        match data.read().await.insert_courses(&vec![crs]).await {
            Ok((n_crs, n_ch)) => {
                log::trace!(
                    "Inserted {} Cours(es) and {} Chapter(s) into the Data DB.",
                    n_crs, n_ch
                );
            },
            Err(e) => {
                return text_500(Some(e.into()));
            },
        };
    }

    refresh_and_repopulate_courses(glob).await
}

async fn update_course(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let body = match body {
        Some(body) => body,
        None => { return respond_bad_request(
            "Request requires applicaiton/json body with Course details.".to_owned()
        ); },
    };

    let crs: Course = match serde_json::from_str(&body) {
        Ok(crs) => crs,
        Err(e) => {
            log::error!(
                "Error deserializing JSON {:?} as Course: {}",
                &body, &e
            );
            return text_500(Some("Unable to deserialize to Course struct.".to_owned()));
        },
    };

    {
        let glob = glob.read().await;
        let data = glob.data();
        if let Err(e) = data.read().await.update_course(&crs).await {
            return text_500(Some(format!("Unable to update Course: {}", &e)));
        };
    }

    refresh_and_repopulate_courses(glob).await
}

async fn delete_course(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let body = match body {
        Some(body) => body,
        None => { return respond_bad_request(
            "Request requires sym of Course in body.".to_owned()
        ); },
    };

    {
        let glob = glob.read().await;
        let data = glob.data();
        match data.read().await.delete_course(&body).await {
            Ok((n_crs, n_ch)) => {
                log::trace!(
                    "Deleted {} Course, {} Chapters from Data DB.",
                    n_crs, n_ch
                );
            },
            Err(e) => {
                return text_500(Some(e.into()));
            },
        };
    }

    refresh_and_repopulate_courses(glob).await
}

async fn add_chapters(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let body = match body {
        Some(body) => body,
        None => { return respond_bad_request(
            "Request requires application/json body with new Chapter info.".to_owned()
        ); },
    };

    let chapters: Vec<Chapter> = match serde_json::from_str(&body) {
        Ok(ch) => ch,
        Err(e) => {
            log::error!(
                "Error deserializing JSON {:?} as Chapter: {}",
                &body, &e
            );
            return text_500(Some("Unable to deserialize to vector of Chapters.".to_owned()));
        }
    };

    {
        let glob = glob.read().await;
        let data = glob.data();
        if let Err(e) = data.read().await.insert_chapters(&chapters).await {
            return text_500(Some(format!(
                "Unable to insert Chapter: {}", &e
            )));
        };
    }

    refresh_and_repopulate_courses(glob).await
}

async fn delete_chapter(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let body = match body {
        Some(body) => body,
        None => { return respond_bad_request(
            "Request requires id of Chapter in body.".to_owned()
        ); },
    };

    let ch_id: i64 = match body.parse() {
        Ok(n) => n,
        Err(e) => {
            return respond_bad_request(format!(
                "Unable to parse body of request {:?} as Chapter id: {}",
                &body, &e
            ));
        },
    };

    {
        let glob = glob.read().await;
        let data = glob.data();
        if let Err(e) = data.read().await.delete_chapter(ch_id).await {
            return text_500(Some(format!(
                "Unable to delete Chapter: {}", &e
            )));
        };
    }

    refresh_and_repopulate_courses(glob).await
}

async fn update_chapter(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let body = match body {
        Some(body) => body,
        None => { return respond_bad_request(
            "Request requires application/json body with Chapter details.".to_owned()
        ); },
    };

    let ch: Chapter = match serde_json::from_str(&body) {
        Ok(ch) => ch,
        Err(e) => {
            log::error!(
                "Error deserializing JSON {:?} as Chapter: {}",
                &body, &e
            );
            return text_500(Some("Unable to deserialize to Chapter struct.".to_owned()));
        }
    };

    {
        let glob = glob.read().await;
        let data = glob.data();
        if let Err(e) = data.read().await.update_chapter(&ch).await {
            return text_500(Some(format!(
                "Unable to update Chapter: {}", &e
            )));
        };
    }

    refresh_and_repopulate_courses(glob).await
}

//
//
// This section is for dealing with the CALENDAR.
//
//

async fn update_calendar(body: Option<String>, glob: Arc<RwLock<Glob>>) -> Response {
    let body: String = match body {
        Some(body) => body,
        None => { return respond_bad_request(
            "Request requires application/json body with Array of date strings.".to_owned()
        ); },
    };

    let date_strs: Vec<&str> = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            log::error!(
                "Error deserializing JSON {:?} as Vector of &str: {}",
                &body, &e
            );
            return text_500(Some("Unable to deserialize to Vector of &str.".to_owned()));
        }
    };

    let mut dates: Vec<Date> = Vec::with_capacity(date_strs.len());
    for s in date_strs.iter() {
        match Date::parse(s, DATE_FMT) {
            Ok(d) => { dates.push(d); },
            Err(e) => {
                log::error!(
                    "Error parsing {:?} as Date: {}", s, &e
                );
                return text_500(Some(format!("Unable to parse {:?} as Date.", s)));
            },
        }
    }

    {
        let glob = glob.read().await;
        let data = glob.data();
        let reader = data.read().await;
        if let Err(e) = reader.set_calendar(&dates).await {
            return text_500(Some(format!("Unable to update calendar: {}", &e)));
        }
    }

    refresh_and_repopulate_calendar(glob).await
}