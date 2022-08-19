/*!
Subcrate for interoperation with Teacher users.
*/
use axum::{
    extract::Extension,
    http::header::{HeaderMap, HeaderName, HeaderValue},
    Json,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::Date;
use tokio::sync::RwLock;

use crate::{
    auth, auth::AuthResult,
    DATE_FMT,
    config::Glob,
    course::{Chapter, Course},
    pace::{BookCh, Pace, Source},
    user::*,
};
use super::*;

pub async fn login(
    t: Teacher,
    form: LoginData,
    glob: Arc<RwLock<Glob>>
) -> Response {
    log::trace!(
        "teacher::login( {:?}, ... , [ glob ]) called.",
        &t.base.uname
    );

    let auth_response = {
        glob.read().await.auth().read().await.check_password_and_issue_key(
            &t.base.uname,
            &form.password,
            &t.base.salt
        ).await
    };

    let auth_key = match auth_response {
        Err(e) => {
            log::error!(
                "auth::Db::check_password_and_issue_key( {:?}, {:?}, {:?} ): {}",
                &t.base.uname, &form.password, &t.base.salt, &e
            );

            return html_500();
        },
        Ok(AuthResult::Key(k)) => k,
        Ok(AuthResult::BadPassword) => { return respond_bad_password(); },
        Ok(x) => {
            log::warn!(
                "auth::Db::check_password_and_issue_key( {:?}, {:?}. {:?} returned {:?}, which shouldn't ever happen.",
                &t.base.uname, &form.password, &t.base.salt, &x
            );

            return respond_bad_password();
        },
    };

    let data = json!({
        "uname": &t.base.uname,
        "key": &auth_key,
        "name": &t.name
    });

    serve_template(
        StatusCode::OK,
        "teacher",
        &data,
        vec![]
    )    
}

pub async fn api(
    headers: HeaderMap,
    _body: Option<String>,
    Extension(glob): Extension<Arc<RwLock<Glob>>>
) -> Response {

    let uname: &str = match headers.get("x-camp-uname") {
        Some(uname) => match uname.to_str() {
            Ok(s) => s,
            Err(_) => { return text_500(None); },
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
        User::Teacher(_) => { /* Okay, approved, you can be here. */ },
        _ => {
            return (
                StatusCode::FORBIDDEN,
                "Who is this? What's you're operating number?".to_owned(),
            ).into_response();
        },
    }

    let action = match headers.get("x-camp-action") {
        Some(act) => match act.to_str() {
            Ok(s) => s,
            Err(_) => { return respond_bad_request(
                "x-camp-action header unrecognizable.".to_owned()
            ); },
        },
        None => { return respond_bad_request(
            "Request must have an x-camp-action header.".to_owned()
        ); },
    };

    match action {
        "populate-courses" => populate_courses(glob.clone()).await,
        "populate-goals" => populate_goals(&headers, glob.clone()).await,
        x => respond_bad_request(
            format!("{:?} is not a recognized x-camp-action value.", &x)
        ),
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct ChapterData<'a> {
    id: i64,
    sym: &'a str,
    seq: i16,
    title: &'a str,
    subject: Option<&'a str>,
    weight: f32,
}

#[derive(Debug, Deserialize, Serialize)]
struct CourseData<'a> {
    id: i64,
    sym: &'a str,
    book: &'a str,
    title: &'a str,
    level: f32,
    weight: f32,
    chapters: Vec<ChapterData<'a>>
}

impl<'a> CourseData<'a> {
    fn from_course(crs: &'a Course) -> Result<CourseData<'a>, String> {
        let tot_wgt = match crs.weight {
            None => { return Err(format!(
                "Course {:?} ({}) doesn't have an initialized total weight.",
                &crs.sym, &crs.title
            )); },
            Some(w) => {
                if w < 0.0001 {
                    return Err(format!(
                        "Total course weight of course {:?} ({}) risks division by zero.",
                        &crs.sym, &crs.title
                    ));
                } else {
                    w
                }
            }
        };

        let chapters: Vec<ChapterData<'a>> = crs.all_chapters()
            .map(|ch| ChapterData {
                id: ch.id,
                sym: &crs.sym,
                seq: ch.seq,
                title: &ch.title,
                subject: match &ch.subject {
                    Some(s) => Some(s.as_str()),
                    None => None,
                },
                weight: ch.weight / tot_wgt,
            }).collect();
        
        let crsd = CourseData {
            id: crs.id,
            sym: &crs.sym,
            book: &crs.book,
            title: &crs.title,
            level: crs.level,
            weight: tot_wgt,
            chapters
        };
        Ok(crsd)
    }
}

async fn populate_courses(glob: Arc<RwLock<Glob>>) -> Response {
    let glob = glob.read().await;

    let mut course_data: Vec<CourseData> = Vec::with_capacity(glob.courses.len());
    for (_, crs) in glob.courses.iter() {
        match CourseData::from_course(crs) {
            Ok(crsd) => { course_data.push(crsd); },
            Err(e) => {
                log::warn!(
                    "Error serializing: {}", &e
                );
            }
        }
    }

    (
        StatusCode::OK,
        [(
            HeaderName::from_static("x-camp-action"),
            HeaderValue::from_static("populate-courses")
        )],
        Json(&course_data)
    ).into_response()
}

#[derive(Debug, Deserialize, Serialize)]
struct GoalData<'a> {
    id: i64,
    #[serde(skip_serializing)]
    uname: &'a str,
    sym: &'a str,
    seq: i16,
    rev: bool,
    inc: bool,
    due: Option<String>,
    done: Option<String>,
    tries: Option<i16>,
    weight: f32,
    score: Option<&'a str>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PaceData<'a> {
    uname: &'a str,
    last: &'a str,
    rest: &'a str,
    tuname: &'a str,
    total_weight: f32,
    due_weight: f32,
    done_weight: f32,
    goals: Vec<GoalData<'a>>,
    /// Fall/Spring exams
    fex: Option<&'a str>,
    sex: Option<&'a str>,
    /// Fall/Spring notices
    fnot: i16,
    snot: i16,
}

impl<'a> PaceData<'a> {
    pub fn from_pace(pcal: &'a Pace) -> Result<PaceData, String> {
        let mut goals: Vec<GoalData> = Vec::with_capacity(pcal.goals.len());
        for g in pcal.goals.iter() {
            let src = match &g.source {
                Source::Book(bch) => bch,
                _ => { return Err(format!(
                    "Student {:?} ({}, {}) has Goal w/ (unsupported) custom Source.",
                    &pcal.student.base.uname, &pcal.student.last, &pcal.student.rest
                )); },
            };

            let gdat = GoalData {
                id: g.id,
                uname: "",
                sym: &src.sym,
                seq: src.seq,
                rev: g.review,
                inc: g.incomplete,
                due: g.due.map(|d| d.to_string()),
                done: g.done.map(|d| d.to_string()),
                tries: g.tries,
                weight: g.weight,
                score: g.score.as_deref(),
            };

            goals.push(gdat);
        }

        let pdat = PaceData {
            uname: &pcal.student.base.uname,
            last: &pcal.student.last,
            rest: &pcal.student.rest,
            tuname: &pcal.teacher.base.uname,
            total_weight: pcal.total_weight,
            due_weight: pcal.due_weight,
            done_weight: pcal.done_weight,
            goals,
            fex: pcal.student.fall_exam.as_deref(),
            sex: pcal.student.spring_exam.as_deref(),
            fnot: pcal.student.fall_notices,
            snot: pcal.student.spring_notices,
        };

        Ok(pdat)
    }
}

async fn populate_goals(headers: &HeaderMap, glob: Arc<RwLock<Glob>>) -> Response {

    let uname: &str = match headers.get("x-camp-uname") {
        Some(uname) => match uname.to_str() {
            Ok(s) => s,
            Err(_) => { return text_500(None); },
        },
        None => { return text_500(None); },
    };

    let pace_cals = match glob.read().await.get_paces_by_teacher(uname).await {
        Ok(goals) => goals,
        Err(e) => { return text_500(Some(format!("{}", &e))); },
    };

    let mut pace_data: Vec<PaceData> = Vec::with_capacity(pace_cals.len());
    for p in pace_cals.iter() {
        match PaceData::from_pace(p) {
            Ok(pd) => { pace_data.push(pd); },
            Err(e) => {
                log::error!("{}", &e);
            }
        }
    }

    (
        StatusCode::OK,
        [(
            HeaderName::from_static("x-camp-action"),
            HeaderValue::from_static("populate-goals")
        )],
        Json(pace_data)
    ).into_response()
}

async fn update_pace(pcal: &Pace) -> Response {
    let cal = match PaceData::from_pace(pcal) {
        Ok(cal) => cal,
        Err(e) => { return text_500(Some(format!(
            "Unable to serialize response: {}", &e
        ))); },
    };

    (
        StatusCode::OK,
        [(
            HeaderName::from_static("x-camp-action"),
            HeaderValue::from_static("update-pace")
        )],
        Json(cal)
    ).into_response()
}