/*!
Subcrate for generation of "Boss" page.
*/
use core::fmt::Write as CoreWrite;
use std::io::Write as IoWrite;
use std::sync::Arc;

use futures::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use smallstr::SmallString;
use smallvec::SmallVec;
use time::{
    Date,
    format_description::FormatItem,
    macros::format_description,
};
use tokio::sync::RwLock;

use crate::{
    auth, auth::AuthResult,
    config::Glob,
    MiniString,
    pace::{
        Goal, GoalDisplay, GoalStatus,
        maybe_parse_score_str,
        Pace, PaceDisplay, RowDisplay, Source, SummaryDisplay,
    },
    user::{BaseUser, User, Student},
};
use super::*;

type SMALLSTORE = [u8; 16];
type MEDSTORE = [u8; 32];

const DATE_FMT: &[FormatItem] = format_description!("[month repr:short] [day]");

fn unwrap_string_vector(mut sv: SmallVec<SMALLSTORE>) -> SmallString<SMALLSTORE> {
    let length = sv.len();
    for _ in length..sv.capacity() { sv.push(b' '); }
    let arr = sv.into_inner().unwrap();
    let mut sstr = unsafe { SmallString::from_buf_unchecked(arr) };
    sstr.truncate(length);
    sstr
}

pub async fn login(
    base: BaseUser,
    form: LoginData,
    glob: Arc<RwLock<Glob>>
) -> Response {
    log::trace!(
        "boss::login( {:?}, {:?}, [ Glob ] ) called.", &base, &form
    );

    match glob.read().await.auth().read().await.check_password(
            &base.uname,
            &form.password,
            &base.salt
        ).await {
        Err(e) => {
            log::error!(
                "auth:Db::check_password( {:?}, {:?}, {:?} ): {}",
                &base.uname, &form.password, &base.salt, &e
            );
            return html_500();
        },
        Ok(AuthResult::Ok) => { /* This is the happy path. */},
        Ok(AuthResult::BadPassword) => { return respond_bad_password(&base.uname); },
        Ok(x) => {
            log::warn!(
                "auth::Db::check_password( {:?}, {:?}, {:?} ) returned {:?}, which shouldn't happen.",
                &base.uname, &form.password, &base.salt, &x
            );
            return respond_bad_password(&base.uname);
        },
    }

    let calendar_string = match make_boss_calendars(glob.clone()).await {
        Ok(s) => s,
        Err(e) => {
            log::error!(
                "Error attempting to write boss calendars: {}", &e
            );
            return respond_login_error(StatusCode::INTERNAL_SERVER_ERROR, &e);
        },
    };

    let data = json!({
        "uname": &base.uname,
        "calendars": calendar_string,
    });

    serve_raw_template(
        StatusCode::OK,
        "boss",
        &data,
        vec![]
    )
}

#[derive(Serialize)]
struct GoalData<'a> {
    row_class: &'a str,
    row_bad: &'a str,
    course: &'a str,
    book: &'a str,
    chapter: &'a str,
    review: &'a str,
    incomplete: &'a str,
    due: MiniString<SMALLSTORE>,
    done: MiniString<SMALLSTORE>,
    score: MiniString<SMALLSTORE>,
}

fn write_cal_goal<W: Write>(
    g: &GoalDisplay,
    glob: &Glob,
    mut buff: W
) -> Result<(), String> {

    let row_class = match g.status {
        GoalStatus::Done => "done",
        GoalStatus::Late => "late",
        GoalStatus::Overdue => "overdue",
        GoalStatus::Yet => "yet",
    };

    let row_bad = if g.inc && g.done.is_none() {
        " bad"
    } else {
        ""
    };

    let review = if g.rev { " R " } else { "" };
    let incomplete = if g.inc { " I " } else { "" };

    let mut due: MiniString<SMALLSTORE> = MiniString::new();
    if let Some(d) = g.due {
        d.format_into(&mut due, DATE_FMT).map_err(|e| format!(
            "Error writing due date {:?}: {}", &d, &e
        ))?;
    }

    let mut done: MiniString<SMALLSTORE> = MiniString::new();
    if let Some(d) = g.done {
        d.format_into(&mut done, DATE_FMT).map_err(|e| format!(
            "Error writing done date {:?}: {}", &d, &e
        ))?;
    }

    let mut score: MiniString<SMALLSTORE> = MiniString::new();
    if let Some(f) = g.score {
        let pct = (100.0 * f).round() as i32;
        write!(&mut score, "{} %", &pct).map_err(|e| format!(
            "Error writing score {:?}: {}", &pct, &e
        ))?;
    }

    let data = GoalData {
        row_class, row_bad, review, incomplete, due, done, score,
        course: g.course,
        book: g.book,
        chapter: g.title,
    };

    write_raw_template("boss_goal_row", &data, buff)
}

#[derive(Serialize)]
struct PaceData<'a> {
    table_class: SmallString<MEDSTORE>,
    uname: &'a str,
    name: String,
    tuname: &'a str,
    teacher: &'a str,
    n_done: usize,
    n_due: usize,
    lag: i32,
    lagstr: SmallString<SMALLSTORE>,
    rows: String,
}

fn write_cal_table<W: Write>(
    p: &Pace,
    glob: &Glob,
    mut buff: W
) -> Result<(), String> {
    log::trace!("make_cal_table( [ {:?} Pace], [ Glob ] ) called.", &p.student.base.uname);

    let pd = PaceDisplay::from(p, glob).map_err(|e| format!(
        "Error generating PaceDisplay for {:?}: {}\npace data: {:?}",
        &p.student.base.uname, &e, &p
    ))?;

    let mut table_class: SmallString<MEDSTORE> = SmallString::from_str("cal");
    if pd.previously_inc {
        write!(&mut table_class, " inc").map_err(|e| format!("Error writing table class: {}", &e))?;
    }
    if pd.weight_done < pd.weight_due {
        write!(&mut table_class, " lag").map_err(|e| format!("Error writing table class: {}", &e))?;
    }
    if pd.n_done < pd.n_due {
        write!(&mut table_class, " count").map_err(|e| format!("Error writing table class: {}", &e))?;
    }

    let name = format!("{}, {}", pd.last, pd.rest);

    let lag = if pd.weight_scheduled.abs() < 0.001 {
        0
    } else {
        (100.0 * (pd.weight_done - pd.weight_due) / pd.weight_scheduled) as i32
    };
    let mut lagstr: SmallString<SMALLSTORE> = SmallString::new();
    write!(&mut lagstr, "{:+}%", &lag).map_err(|e| format!("Error writing lag string: {}", &e))?;

    let mut rows: Vec<u8> = Vec::new();
    for row in pd.rows.iter() {
        if let RowDisplay::Goal(g) = row {
            write_cal_goal(&g, glob, &mut rows).map_err(|e| format!(
                "Error writing cal for {:?}: {}", &p.student.base.uname, &e
            ))?;
        }
    }
    let rows = String::from_utf8(rows).map_err(|e| format!(
        "Calendar rows for {:?} not UTF-8 for some reason: {}", &p.student.base.uname, &e
    ))?;

    let data = PaceData {
        table_class, name, lag, lagstr, rows,
        uname: pd.uname,
        tuname: pd.tuname,
        teacher: pd.teacher,
        n_done: pd.n_done,
        n_due: pd.n_due
    };

    write_raw_template("boss_pace_table", &data, &mut buff)

/*     let semf_end = match glob.dates.get("end-fall") {
        Some(d) => d,
        None => { return Err("Special date \"end-fall\" not set by Admin.".to_owned()); },
    };

    let mut semf_done: Vec<&Goal> = Vec::with_capacity(p.goals.len());
    let mut sems_done: Vec<&Goal> = Vec::with_capacity(p.goals.len());

    let mut semf_inc = false;
    let mut sems_inc = false;
    let mut prev_year_inc: bool = false;

    let mut n_due: usize = 0;
    let mut n_done: usize = 0;
    let mut weight_due: f32 = 0.0;
    let mut weight_done: f32 = 0.0;

    for g in p.goals.iter() {
        if let Some(d) = &g.due {
            if d < today {
                n_due += 1;
                weight_due += g.weight;
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
            if d < &semf_end {
                semf_done.push(g);
            } else {
                sems_done.push(g);
            }
            if d < today {
                n_done += 1;
                weight_done += g.weight;
            }
        } else if g.incomplete {
            prev_year_inc = true;
        }
    }

    let semf_last_id = semf_done.last().map(|g| g.id);
    let sems_last_id = sems_done.last().map(|g| g.id);

    let mut goals_buff: Vec<u8> = Vec::new();

    for g in p.goals.iter() {
        let source = match &g.source {
            Source::Book(bch) => bch,
            _ => { return Err("Custom chapters not yet supported.".to_owned()); }
        };

        let crs = match glob.course_by_sym(&source.sym) {
            Some(crs) => crs,
            None => { return Err(format!("No Course with symbol {:?}!", source.sym)); }
        };

        let course = &crs.title;
        let chapter = match crs.chapter(source.seq) {
            Some(ch) => &ch.title,
            None => "",
        };
        let book = crs.book.as_str();

        let review = match g.review {
            true => " R",
            false => "",
        };
        let incomplete = match g.incomplete {
            true => " I",
            false => "",
        };

        let mut due: SmallVec<SMALLSTORE> = SmallVec::new();
        let mut done: SmallVec<SMALLSTORE> = SmallVec::new();
        let mut score: SmallVec<SMALLSTORE> = SmallVec::new();

        if let Some(d) = g.due {
            if let Err(e) = d.format_into(&mut due, &DATE_FMT) {
                return Err(format!("Error formatting due date: {}", &e));
            }
        }
        if let Some(d) = g.done {
            if let Err(e) = d.format_into(&mut done, &DATE_FMT) {
                return Err(format!("Error formatting done date: {}", &e));
            }
        }

        if let Some(f) = maybe_parse_score_str(g.score.as_deref())? {
            write!(&mut score, "{:.0}", f * 100.0).map_err(|e| format!(
                "Error writing score: {}", &e
            ))?;
        }

        let due = unwrap_string_vector(due);
        let done = unwrap_string_vector(done);
        let score = unwrap_string_vector(score);

        let row_class = match &g.due {
            Some(due) => match &g.done {
                Some(done) => if due < done {
                    "late"
                } else {
                    "done"
                },
                None => if due < today {
                    "overdue"
                } else {
                    "yet"
                }
            },
            None => match &g.done {
                Some(_) => "done",
                None => "yet",
            }
        };

        let row_bad = if g.incomplete && g.done.is_none() {
            " bad"
        } else {
            ""
        };

        let data = GoalData {
            row_class, row_bad, course, book, chapter,
            review, incomplete, due, done ,score
        };

        write_raw_template("boss_goal_row", &data, &mut goals_buff)
            .map_err(|e| format!(
                "Error writing goal for {:?}: {}",
                &p.student.base.uname, &e
            ))?;
    }

    let name = format!("{}, {}", &p.student.last, &p.student.rest);
    let uname = p.student.base.uname.as_str();
    let tuname = p.teacher.base.uname.as_str();
    let teacher = p.teacher.name.as_str();
    let lag = if p.total_weight.abs() < 0.001 {
        0
    } else {
        (100.0 * (weight_done - weight_due) / p.total_weight).round() as i32
    };
    let mut lagstr: SmallString<SMALLSTORE> = SmallString::new();
    write!(&mut lagstr, "{:+}%", &lag).map_err(|e| format!("Error writing table: {}", &e))?;

    let rows = String::from_utf8(goals_buff)
        .map_err(|e| format!("Calendar rows not valid UTF-8: {}", &e))?;
    
    let mut table_class: SmallString<MEDSTORE> = SmallString::from_str("cal");
    if prev_year_inc {
        write!(&mut table_class, " inc").map_err(|e| format!("Error writing table: {}", &e))?;
    }
    if lag < 0 {
        write!(&mut table_class, " lag").map_err(|e| format!("Error writing table: {}", &e))?;
    }
    if n_done < n_due {
        write!(&mut table_class, " count").map_err(|e| format!("Error writing table: {}", &e))?;
    }
    
    let data = PaceData {
        table_class, uname, name, tuname, teacher,
        n_done, n_due, lag, lagstr, rows
    };

    write_raw_template("boss_pace_table", &data, &mut buff) */
}

pub async fn make_boss_calendars(
    glob: Arc<RwLock<Glob>>
) -> Result<String, String> {
    log::trace!("make_boss_page( [ Glob ] ) called.");

    let glob = glob.read().await;
    let tunames: Vec<&str> = glob.users.iter()
        .map(|(uname, user)| match user {
            User::Teacher(_) => Some(uname),
            _ => None,
        }).filter(|opt| opt.is_some())
        .map(|ok| ok.unwrap().as_str())
        .collect();
    
    let n_students: usize = glob.users.iter()
        .map(|(_, u)| match u {
            User::Student(_) => true,
            _ => false,
        }).filter(|b| *b)
        .count();
    
    let mut paces: Vec<Pace> = Vec::with_capacity(n_students);
    {
        let mut retrievals = FuturesUnordered::new();
        for tuname in tunames.iter() {
            retrievals.push(
                glob.get_paces_by_teacher(tuname)
            );
        }

        while let Some(res) = retrievals.next().await {
            match res {
                Ok(mut pace_vec) => { paces.extend(pace_vec.drain(..)); },
                Err(e) => { return Err(format!(
                    "Error retrieving goals from database: {}", &e
                )); },
            }
        }
    }

    let mut buff: Vec<u8> = Vec::new();

    for p in paces.iter() {
        if let Err(e) = write_cal_table(
            p, &glob, &mut buff
        ) {
            return Err(format!(
                "Error generating list of pace calendars: {}", &e
            ));
        }

    }

    let buff = String::from_utf8(buff)
        .map_err(|e| format!("Pace calendar not valid UTF-8: {}", &e))?;

    Ok(buff)
}