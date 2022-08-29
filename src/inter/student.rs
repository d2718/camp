/*!
Displaying individual student calendars.
*/
use std::ops::Deref;

use smallstr::SmallString;
use smallvec::SmallVec;
use time::{
    Date,
    format_description::FormatItem,
    macros::format_description,
};

use crate::{
    MiniString,
    user::Student,
    pace::{Goal, maybe_parse_score_str, Pace, Source},
};

use super::*;

type SMALLSTORE = [u8; 16];
type MEDSTORE = [u8; 32];

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

#[derive(Debug, Serialize)]
struct GoalData<'a> {
    course: &'a str,
    book: &'a str,
    chapter: &'a str,
    subject: &'a str,
    ri: &'a str,
    due: MiniString<SMALLSTORE>,
    due_from: MiniString<SMALLSTORE>,
    done: MiniString<SMALLSTORE>,
    done_from: MiniString<SMALLSTORE>,
    tries: Option<i16>,
    score: Option<u32>,
    goal_class: &'a str,
}

#[derive(Debug, Serialize)]
struct SummaryData<'a> {
    text: &'a str,
    score: MiniString<SMALLSTORE>    
}

enum Sem {
    Fall,
    Spring
}

impl Deref for Sem {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            Sem::Fall => "Fall",
            Sem::Spring => "Spring",
        }
    }
}

pub fn write_goal_row<W: Write>(
    w: W,
    g: &Goal, 
    glob: &Glob,
    today: &Date,
) -> Result<(), String> {
    let source = match &g.source {
        Source::Book(bch) => bch,
        _ => { return Err("Custom Goals not supported.".to_owned()); },
    };

    let crs = match glob.course_by_sym(&source.sym) {
        Some(crs) => crs,
        None => { return Err(format!("No course with symbol {:?}.", &source.sym)); },
    };

    let chp = match crs.chapter(source.seq) {
        Some(chp) => chp,
        None => { return Err(format!(
            "Course {:?} ({}, {}) has no chapter {}.",
            &crs.sym, &crs.title, &crs.book, &source.seq
        )); },
    };

    let course = crs.title.as_str();
    let book = crs.book.as_str();
    let chapter = chp.title.as_str();
    let subject = match &chp.subject {
        Some(s) => s.as_str(),
        None => "",
    };

    let ri = match (g.review, g.incomplete) {
        (false, false) => "",
        (true, false) => " R*",
        (false, true) => " I*",
        (true, true) => " R* I*",
    };

    let mut due: MiniString<SMALLSTORE> = MiniString::new();
    let mut due_from: MiniString<SMALLSTORE> = MiniString::new();
    let mut done: MiniString<SMALLSTORE> = MiniString::new();
    let mut done_from: MiniString<SMALLSTORE> = MiniString::new();
    let mut goal_class = "";

    if let Some(d) = &g.due {
        d.format_into(&mut due, &DATE_FMT).map_err(|e| format!(
            "Failed to format date {:?}: {}", d, &e
        ))?;
        let delta = (*d - *today).whole_days();
        if delta > 0 {
            write!(&mut due_from, "in {} days", &delta).map_err(|e| e.to_string())?;
        } else if delta < 0 {
            write!(&mut due_from, "{} days ago", -delta).map_err(|e| e.to_string())?;
        } else {
            write!(&mut due_from, "today").map_err(|e| e.to_string())?;
        }

        if let Some(n) = &g.done {
            n.format_into(&mut done, &DATE_FMT).map_err(|e| format!(
                "Failed to format date {:?}: {}", n, &e
            ))?;
            let delta = (*d - *n).whole_days();
            if delta > 0 {
                write!(&mut done_from, "{} days early", &delta).map_err(|e| e.to_string())?;
                goal_class = "done";
            } else if delta < 0 {
                write!(&mut done_from, "{} days late", -delta).map_err(|e| e.to_string())?;
                goal_class = "late";
            } else {
                write!(&mut done_from, "on time").map_err(|e| e.to_string())?;
                goal_class = "done";
            }
        } else {
            if today > d {
                goal_class = "overdue";
            } else {
                goal_class = "yet";
            }
        }
    } else {
        if let Some(n) = &g.done {
            n.format_into(&mut done, &DATE_FMT).map_err(|e| format!(
                "Failed to format date {:?}: {}", n, &e
            ))?;
            goal_class = "done";
        } else {
            goal_class = "yet";
        }
    }

    let tries = g.tries;
    let score =  maybe_parse_score_str(g.score.as_deref())?
        .map(|f| (f * 100.0).round() as u32 );
    
    let data = GoalData {
        course, book, chapter, subject, ri, due, due_from,
        done, done_from, tries, score, goal_class
    };

    write_template("student_goal_row", &data, w)
        .map_err(|e| format!(
            "Error writing goal {:?}: {}", g, &e
        ))
}

fn write_summary(
    buff: &mut Vec<u8>,
    sem: Sem,
    test_avg: f32,
    sem_inc: bool,
    s: &Student
) -> Result<(), String> {
    let mut label: MiniString<MEDSTORE> = MiniString::new();
    write!(&mut label, "{} Test Average:", sem.deref()).map_err(|e| format!(
        "Error writing label: {}", &e
    ))?;
    let mut score: MiniString<SMALLSTORE> = MiniString::new();
    let int_avg = (100.0 * test_avg).round() as i32;
    write!(&mut score, "{}%", &int_avg).map_err(|e| format!(
        "Error writing score {:?}: {}", &int_avg, &e
    ))?;
    if sem_inc {
        write!(&mut score, " (I)").map_err(|e| format!(
            "Error writing to score string: {}", &e
        ))?;
    }
    let data = SummaryData{ text: label.as_str(), score };
    write_template("summary_row", &data, &mut *buff).map_err(|e| format!(
        "Error writing summary row w/data {:?}: {}", &data, &e
    ))?;

    // If there's an exam score, write the rest of the summary data.
    let maybe_score = match sem {
        Sem::Fall => &s.fall_exam,
        Sem::Spring => &s.spring_exam,
    };
    if let Some(f) = maybe_parse_score_str(maybe_score.as_deref())? {
        let int_score = (100.0 * f).round() as i32;
        let mut score: MiniString<SMALLSTORE> = MiniString::new();
        write!(&mut score, "{}%", &int_score).map_err(|e| format!(
            "Error writing score {:?}: {}", &int_score, &e
        ))?;
        let data = SummaryData{ text: "Final Exam:", score };
        write_template("summary_row", &data, &mut *buff).map_err(|e| format!(
            "Error writing summary row w/data {:?}: {}", &data, &e
        ))?;

        let exam_frac = match sem {
            Sem::Fall => s.fall_exam_fraction,
            Sem::Spring => s.spring_exam_fraction,
        };
        let sem_final = (exam_frac * f) + ((1.0 - exam_frac) * test_avg);
        let mut sem_pct = 100.0 * sem_final;

        // Only write notices row if there are notices.
        let notices = match sem {
            Sem::Fall => s.fall_notices,
            Sem::Spring => s.spring_notices,
        };
        if notices > 0 {
            let mut label: MiniString<MEDSTORE> = MiniString::new();
            write!(&mut label, "Notices ({}):", &notices).map_err(|e| format!(
                "Error writing label: {}", &e
            ))?;
            let mut score: MiniString<SMALLSTORE> = MiniString::new();
            write!(&mut score, "-{}%", &notices).map_err(|e| format!(
                "Error writing notices # {:?}: {}", &notices, &e
            ))?;
            let data = SummaryData{ text: label.deref(), score };
            write_template("summary_row", &data, &mut *buff).map_err(|e| format!(
                "Error writing summary row w/data {:?}: {}", &data, &e
            ))?;

            sem_pct = sem_pct - (notices as f32);
        }

        let int_pct = sem_pct.round() as i32;
        let mut label: MiniString<MEDSTORE> = MiniString::new();
        write!(&mut label, "{} Semester Grade:", sem.deref()).map_err(|e| format!(
            "Error writing label: {}", &e
        ))?;
        let mut score: MiniString<SMALLSTORE> = MiniString::new();
        write!(&mut score, "{}%", &int_pct).map_err(|e| format!(
            "Error writing score {:?}: {}", &sem_pct, &e
        ))?;
        if sem_inc {
            write!(&mut score, " (I)").map_err(|e| format!(
                "Error writing to score string: {}", &e
            ))?;
        }
        let data = SummaryData{ text: label.as_str(), score };
        write_template("summary_row", &data, &mut *buff).map_err(|e| format!(
            "Error writing summary row w/data {:?}: {}", &data, &e
        ))?;
    }

    Ok(())
}

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
    let mut n_scheduled: usize = 0;
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
            n_scheduled += 1;
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

            n_done += 1;
        }

        if g.incomplete {
            need_inc_footnote = true;
        }
        if g.review {
            need_rev_footnote = true;
        }
    }

    let mut goals_buff: Vec<u8> = Vec::new();

    for g in p.goals.iter() {
        if let Err(e) = write_goal_row(&mut goals_buff, g, &glob, &today) {
            log::error!(
                "Error writing Goal {:?} row: {}", g, &e
            );
            return html_500();
        }

        // Write Fall Semester summary data, if applicable.
        if let Some(id) = semf_last_id {
            if id == g.id && semf_done > 0 {
                let semf_frac = semf_total / (semf_done as f32);
                if let Err(e) = write_summary(&mut goals_buff, Sem::Fall, semf_frac, semf_inc, &s) {
                    log::error!("Error generating Fall summary: {}", &e);
                    return html_500();
                }
            }
        }

        // Write Spring Semester summary data, if applicable.
        if let Some(id) = sems_last_id {
            if id == g.id && sems_done > 0 {
                let sems_frac = sems_total / (sems_done as f32);
                if let Err(e) = write_summary(&mut goals_buff, Sem::Spring, sems_frac, sems_inc, &s) {
                    log::error!("Error generating Spring summary: {}", &e);
                    return html_500();
                }            
            }
        }
    }

    let rows = unsafe { String::from_utf8_unchecked(goals_buff) };

    let rev_foot = if need_rev_footnote {
        "*R indicates previously-completed material that requires review."
    } else {
        ""
    };
    let inc_foot = if need_inc_footnote {
        "*I indicates material incomplete from the prior academic year."
    } else {
        ""
    };

    let data = json!({
        "name": format!("{} {}", &s.rest, &s.last),
        "uname": &s.base.uname,
        "teacher": &p.teacher.name,
        "temail": &p.teacher.base.email,
        "n_done": n_done,
        "n_due": n_due,
        "n_total": n_scheduled,
        "rows": rows,
        "rev_foot": rev_foot,
        "inc_foot": inc_foot,
    });

    serve_raw_template(
        StatusCode::OK,
        "student",
        &data,
        vec![]
    )
}