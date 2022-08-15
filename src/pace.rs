/*!
The `Goal` struct and Pace calendars.
*/
use std::{
    cmp::Ordering,
    collections::HashMap,
    io::Read,
};

use time::{Date, Month};

use crate::{
    config::Glob,
    course::Course,
    user::{Student, Teacher, User},
};

#[derive(Clone, Debug)]
pub struct BookCh {
    pub sym: String,
    pub seq: i16,
    // Gets set in the constructor of the `Pace` calendar.
    pub level: f32,
}

impl PartialEq for BookCh {
    fn eq(&self, other: &Self) -> bool {
        self.sym == other.sym && self.seq == other.seq
    }
}
impl Eq for BookCh {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CustomCh(i64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Source {
    Book(BookCh),
    Custom(CustomCh)
}

#[derive(Clone, Debug)]
pub struct Goal {
    pub id: i64,
    pub uname: String,
    pub source: Source,
    pub review: bool,
    pub incomplete: bool,
    pub due: Option<Date>,
    pub done: Option<Date>,
    pub tries: i16,
    // Should get set in the constructor of the `Pace` calendar.
    pub weight: f32,
    pub score: Option<String>,
}

impl PartialEq for Goal {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && &self.uname == &other.uname
            && &self.source == &other.source
            && self.review == other.review
            && self.incomplete == other.incomplete
            && &self.due == &other.due
            && &self.done == &other.done
            && self.tries == other.tries
            && &self.score == &other.score
    }
}

impl Eq for Goal {}

fn blank_means_none(s: Option<&str>) -> Option<&str> {
    match s {
        Some(s) => match s.trim() {
            "" => None,
            x => Some(x),
        },
        None => None,
    }
}

impl Goal {
    /**
    Goal .csv rows should look like this

    ```csv
    #uname, sym, seq,     y, m,  d, rev, inc
    jsmith, pha1,  3, 2022, 09, 10,   x,
          ,     ,  9,     ,   , 28,    ,  x
    ```

    Columns `uname`, `sym`, `y`, `m` all default to the value of the previous
    goal, so to save work, you don't need to include them if they're the same
    as the previous line.

    Columns `rev` and `inc` are considered `true` if they have any text
    whatsoever.
     */
    pub fn from_csv_line(
        row: &csv::StringRecord,
        prev: Option<&Goal>
    ) -> Result<Goal, String> {
        log::trace!("Goal::from_csv_line( {:?} ) called.", row);

        let uname = match blank_means_none(row.get(0)) {
            Some(s) => s.to_owned(),
            None => match prev {
                Some(g) => g.uname.clone(),
                None => { return Err("No uname".into()); },
            },
        };

        let seq: i16 = match blank_means_none(row.get(2)) {
            Some(s) => match s.parse() {
                Ok(n) => n,
                Err(_) => { return Err(format!("Unable to parse {:?} as number.", s)); },
            },
            None => { return Err("No chapter number.".into()); },
        };

        let book_ch = match blank_means_none(row.get(1)) {
            Some(s) => BookCh { sym: s.to_owned(), seq, level: 0.0 },
            None => match prev {
                Some(g) => match &g.source {
                    Source::Book(bch) => BookCh { sym: bch.sym.clone(), seq, level: 0.0 },
                    Source::Custom(_) => { return Err("No course symbol.".into()); },
                },
                None => { return Err("No course symbol".into()); },
            },
        };

        let y: i32 = match blank_means_none(row.get(3)) {
            Some(s) => match s.parse() {
                Ok(n) => n,
                Err(_) => { return Err(format!("Unable to parse {:?} as year.", s)); },
            },
            None => match prev {
                Some(g) => match g.due {
                    Some(d) => d.year(),
                    None => { return Err("No year".into()); }
                },
                None => { return Err("No year".into()); },
            }
        };

        let m: Month = match blank_means_none(row.get(4)) {
            Some(s) => match s.parse::<u8>() {
                Ok(n) => match Month::try_from(n-1) {
                    Ok(m) => m,
                    Err(_) => { return Err(format!("Not an appropriate Month value: {}", n)); },
                },
                Err(_) => { return Err(format!("Unable to parse {:?} as month number.", s)); },
            },
            None => match prev {
                Some(g) => match g.due {
                    Some(d) => d.month(),
                    None => { return Err("No month".into()); },
                },
                None => { return Err("No month".into()); },
            },
        };

        let d: u8 = match blank_means_none(row.get(5)) {
            Some(s) => match s.parse() {
                Ok(n) => n,
                Err(_) => { return Err(format!("Unable to parse {:?} as day number.", s)); },
            },
            None => { return Err("No day".into()); },
        };

        let due = match Date::from_calendar_date(y, m, d) {
            Ok(d) => d,
            Err(_) => { return Err(format!("{}-{}-{} is not a valid date", &y, &m, &d)); },
        };

        let review = match blank_means_none(row.get(6)) {
            Some(_) => true,
            None => false,
        };

        let incomplete = match blank_means_none(row.get(7)) {
            Some(_) => true,
            None => false,
        };

        let g = Goal {
            // This doesn't matter; it will be set upon database insertion.
            id: 0,
            uname,
            source: Source::Book(book_ch),
            review,
            incomplete,
            due: Some(due),
            // No goals read from .csv files can possibly be done.
            done: None,
            // Will get set once it's done.
            tries: 0,
            // Will get set in the `Pace` calendar constructror.
            weight: 0.0,
            // Goals read from .csv files should have no score yet.
            score: None,
        };

        Ok(g)
    }
}

impl Ord for Goal {
    fn cmp(&self, other: &Self) -> Ordering {
        use Ordering::*;

        match &self.due {
            Some(d) => match &other.due {
                Some(e) => { return d.cmp(e); },
                None => { return Less; },
            },
            None => match &other.due {
                Some(_) => { return Greater },
                None => { /* fallthrough */ },
            },
        }

        match &self.done {
            Some(d) => match &other.done {
                Some(e) => { return d.cmp(e); },
                None => { return Less; }
            },
            None => match &other.done {
                Some(_) => { return Greater; },
                None => { /* fallthrough */ },
            }
        }

        match &self.source {
            Source::Book(BookCh {sym: _, seq: n, level: slev }) => match &other.source {
                Source::Book(BookCh { sym: _, seq: m, level: olev }) => {
                    if slev < olev {
                        return Less;
                    } else if slev > olev {
                        return Greater;
                    } else {
                        return n.cmp(m);
                    }
                },
                _ => Equal,
            },
            _ => Equal,
        }
    }
}

impl PartialOrd for Goal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
pub struct Pace {
    pub student: Student,
    pub teacher: Teacher,
    pub goals: Vec<Goal>,
    pub total_weight: f32,
    pub due_weight: f32,
    pub done_weight: f32,
}

fn affirm_goal(mut g: Goal, glob: &Glob) -> Result<Goal, String> {
    match glob.users.get(&g.uname) {
        Some(User::Student(_)) => { /* This is the happy path. */ },
        _ => { return Err(format!("{:?} is not a student user name.", &g.uname)); },
    }

    match g.source {
        Source::Book(ref mut b) => {
            let crs = match glob.course_by_sym(&b.sym) {
                Some(c) => c,
                None => { return Err(format!(
                    "{:?} is not a course symbol.", &b.sym
                )); },
            };
            let chp = match crs.chapter(b.seq) {
                Some(ch) => ch,
                None => { return Err(format!(
                    "Course {:?} ({}) does not have a chapter {}.",
                    &b.sym, &crs.title, b.seq
                )); },
            };
            b.level = crs.level;
            let crs_wgt = match crs.weight {
                Some(w) => w,
                None => { return Err(format!(
                    "Course {:?} has not been appropriately initialized! This is a bad bug. Make Dan fix it.",
                    &b.sym
                )); }
            };
            g.weight = chp.weight / crs_wgt;
        },
        Source::Custom(_) => { return Err("Custom Goals not yet supported.".to_owned()); },
    }

    Ok(g)
}

impl Pace {
    pub fn from_csv<R: Read>(
        r: R,
        glob: &Glob
    ) -> Result<Vec<Pace>, String> {
        log::trace!("Pace::from_csv(...) called.");

        let mut csv_reader = csv::ReaderBuilder::new()
            .comment(Some(b'#'))
            .trim(csv::Trim::All)
            .flexible(true)
            .has_headers(false)
            .from_reader(r);
        
        let mut goals_by_uname: HashMap<String, Vec<Goal>> = HashMap::new();

        let mut prev_goal: Option<Goal> = None;
        for (n, res) in csv_reader.records().enumerate() {
            match res {
                Ok(record) => {
                    let res = Goal::from_csv_line(&record, prev_goal.as_ref());
                    match res {
                        Ok(g) => match affirm_goal(g, glob) {
                            Ok(g) => {
                                if let Some(v) = goals_by_uname.get_mut(&g.uname) {
                                    (*v).push(g.clone());
                                } else {
                                    let v = vec![g.clone()];
                                    goals_by_uname.insert(g.uname.clone(), v);
                                }
                                prev_goal = Some(g)
                            }
                            Err(e) => {
                                let estr = match record.position() {
                                    Some(p) => format!(
                                        "Error on line {}: {}", p.line(), &e
                                    ),
                                    None => format!("Error in CSV record {}: {}", &n, &e),
                                };
                                return Err(estr);
                            },
                        },
                        Err(e) => {
                            let estr = match record.position() {
                                Some(p) => format!(
                                    "Error on line {}: {}", p.line(), &e
                                ),
                                None => format!("Error in CSV record {}: {}", &n, &e),
                            };
                            return Err(estr);
                        },
                    }
                },
                Err(e) => {
                    let estr = match e.position() {
                        Some(p) => format!(
                            "Error on line {}: {}", p.line(), &e
                        ),
                        None => format!("Error in CSV record {}: {}", &n, &e),
                    };
                    return Err(estr);
                },
            }
        }

        let mut cals: Vec<Pace> = Vec::with_capacity(goals_by_uname.len());
        for (uname, mut goals) in goals_by_uname.drain() {
            let student = match glob.users.get(&uname) {
                Some(User::Student(s)) => s.clone(),
                _ => { return Err(format!("{:?} is not a Student user name.", &uname)); },
            };
            let teacher = match glob.users.get(&student.teacher) {
                Some(User::Teacher(t)) => t.clone(),
                _ => { return Err(format!(
                    "Student {:?} ({} {}) has nonexistent teachdr {:?} on record.",
                    &uname, &student.rest, &student.last, &student.teacher
                )); },
            };

            goals.sort();
            let total_weight = goals.iter().map(|g| g.weight).sum();

            let p = Pace {
                student,
                teacher,
                goals,
                total_weight,
                due_weight: 0.0,
                done_weight: 0.0,
            };

            cals.push(p);
        }

        Ok(cals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::ensure_logging;
    use crate::*;
    use crate::config::Glob;
    use crate::user::{BaseUser, Role, Teacher, Student, User};

    use std::fs::{File, read_to_string};

    use serial_test::serial;

    static AUTH_CONN: &str = "host=localhost user=camp_test password='camp_test' dbname=camp_auth_test";
    static DATA_CONN: &str = "host=localhost user=camp_test password='camp_test' dbname=camp_store_test";

    static COURSE_FILES: &[&str] = &[
        "test/env/course_0.mix",
        "test/env/course_1.mix",
        "test/env/course_2.mix",
        "test/env/course_3.mix",
    ];

    static BOSS: (&str, &str) = ("boss", "boss@camelthingy.com");
    static TEACHERS: &[(&str, &str, &str)] = &[
        ("bob", "Mr Bob", "bob@school.com"),
        ("sal", "Ms Sally, not Sal Khan", "sally@school.com"),
        ("yak", "Yakov Smirnoff", "yakov@school.com"),
    ];
    const STUDENT_FILE: &str = "test/env/students.csv";
    const GOALS_FILE: &str = "test/env/goals.csv";

    const CONFIG_FILE: &str = "test/env/config.toml";


    async fn init_env() -> Result<Glob, String> {
        ensure_logging();

        let mut g = config::load_configuration(CONFIG_FILE).await.unwrap();

        let courses: Vec<Course> = COURSE_FILES.iter()
            .map(|fname| File::open(fname).unwrap())
            .map(|f| Course::from_reader(f).unwrap())
            .collect();
        
        let boss = BaseUser {
            uname: BOSS.0.to_owned(),
            role: Role::Boss,
            salt: String::new(),
            email: BOSS.1.to_owned(),
        }.into_boss();

        let student_csv = read_to_string(STUDENT_FILE).unwrap();

        let teachers: Vec<User> = TEACHERS.iter()
            .map(|(uname, name, email)|
                BaseUser {
                    uname: uname.to_string(), 
                    role: Role::Teacher,
                    salt: String::new(),
                    email: email.to_string()
                }.into_teacher(name.to_string())
            ).collect();

        {
            let data = g.data();
            data.read().await.insert_courses(&courses).await?;
        }

        g.insert_user(&boss).await.unwrap();
        for u in teachers.iter() {
            g.insert_user(u).await.unwrap();
        }
        g.refresh_users().await.unwrap();
        g.upload_students(&student_csv).await.unwrap();

        g.refresh_courses().await.unwrap();
        g.refresh_users().await.unwrap();

        Ok(g)
    }

    async fn teardown_env(g: Glob) -> Result<(), String> {
        use std::fmt::Write;

        let mut err_msgs = String::new();

        {
            let data = g.data();
            let dread = data.read().await;
            if let Err(e) = dread.nuke_database().await {
                log::error!("Error tearing down data DB: {}", &e);
                writeln!(&mut err_msgs, "Data DB: {}", &e).unwrap();
            }

            let auth = g.auth();
            let aread = auth.read().await;
            if let Err(e) = aread.nuke_database().await {
                log::error!("Error tearing down auth DB: {}", &e);
                writeln!(&mut err_msgs, "Auth DB: {}", &e).unwrap();
            }
        }

        if err_msgs.is_empty() {
            Ok(())
        } else {
            Err(err_msgs)
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_env() {
        let g = init_env().await.unwrap();
        log::info!(
            "Glob has {} courses, {} users.",
            &g.courses.len(), &g.users.len()
        );

        teardown_env(g).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn goals_from_csv() {
        let g = init_env().await.unwrap();
        let goals = Pace::from_csv(
            File::open(GOALS_FILE).unwrap(),
            &g
        ).unwrap();
        log::info!(
            "Read {} courses from test course file {:?}.",
            &goals.len(), GOALS_FILE
        );

        for goal in goals.iter() {
            println!("{:#?}", goal);
        }

    }
}