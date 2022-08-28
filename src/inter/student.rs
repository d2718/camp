/*!
Displaying individual student calendars.
*/
use smallstr::SmallString;
use time::{
    Date,
    format_description::FormatItem,
    macros::format_description,
};

use crate::{
    user::Student,
    pace::maybe_parse_score_str,
};

use super::*;

const DATE_FMT: &[FormatItem] = format_description!("[month repr:short] [day]");

static TEMP_RESPONSE: &str = r#"<!doctype html>
<html>
<head>
    <meta charset="utf-8">
    <title>CAMP | Unimplemented</title>
</head>
<body>
    <h1>Student login as of yet unimplemented.</h1>
    <p>Please come back at a later date.</p>
</body>
</html>"#;

pub async fn login(
    s: Student,
    form: LoginData,
    glob: Arc<RwLock<Glob>>
) -> Response {

    let glob = glob.read().await;
    match glob.auth().read().await.check_password(
        &s.base.uname,
        &form.password,
        &s.base.salt
    ).await {
        Err(e) => {
            log::error!(
                "auth::Db::check_password( {:?}, {:?}, {:?} ) error: {}",
                &s.base.uname, &form.password, &s.base.salt, &e
            );
            return html_500();
        },
        Ok(AuthResult::Ok) => { /* This is the happy path; proceed. */ },
        Ok(AuthResult::BadPassword) => { return respond_bad_password(&s.base.uname); },
        Ok(x) => {
            log::warn!(
                "auth::Db::check_password( {:?}, {:?}, {:?} ) returned {:?}, which shouldn't happen.",
                &s.base.uname, &form.password, &s.base.salt, &x
            );
            return respond_bad_password(&s.base.uname);
        },
    }

    let p = match glob.get_pace_by_student(&s.base.uname).await {
        Ok(p) => p,
        Err(e) => {
            log::error!(
                "Glob::get_pace_by_student( {:?} ) error: {}",
                &s.base.uname, &e
            );
            return html_500();
        },
    };

    let today = crate::now();
    let semf_end = match glob.dates.get("end-fall") {
        Some(d) => d,
        None => {
            log::error!("Date \"end-fall\" not set by Admin.");
            return html_500();
        },
    };

    let mut need_inc_footnote = false;
    let mut need_rev_footnote = false;
    let mut semf_inc = false;
    let mut sems_inc = false;
    let mut n_done: usize = 0;
    let mut n_due: usize = 0;
    let mut semf_total: f32 = 0.0;
    let mut semf_done: usize = 0;
    let mut sems_total: f32 = 0.0;
    let mut sems_done: usize = 0;
    let mut semf_last_id: Option<i64> = None;
    let mut sems_last_id: Option<i64> = None;
    


    for g in p.goals.iter() {
        if let Some(d) = &g.due {
            if d < &today {
                n_due += 1;
            }
            if let &None = &g.done {
                if d < &semf_end {
                    semf_inc = true;
                } else {
                    sems_inc = true;
                }
            }
        }

        if let Some(d) = &g.done {
            let score = match maybe_parse_score_str(g.score.as_deref()) {
                Err(e) => {
                    log::error!(
                        "Error parsing stored score {:?}: {}", &g.score, &e
                    );
                    return html_500();
                },
                Ok(Some(f)) => f,
                Ok(None) => {
                    log::error!(
                        "Goal [ id {} ] has done date but no score.", g.id
                    );
                    return html_500();
                },
            };

            if d < &semf_end {
                semf_total += score;
                semf_done += 1;
                semf_last_id = Some(g.id);
            } else {
                sems_total += score;
                sems_done += 1;
                sems_last_id = Some(g.id);
            }
        }

        if g.incomplete {
            need_inc_footnote = true;
        }
        if g.review {
            need_rev_footnote = true;
        }
    }

    (
        StatusCode::NOT_IMPLEMENTED,
        Html(TEMP_RESPONSE)
    ).into_response()
}