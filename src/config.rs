/*!
Structs to hold configuration data and global variables.
*/
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Write};
use std::io::Cursor;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use handlebars::Handlebars;
use serde::Deserialize;
use time::Date;
use tokio::sync::RwLock;

use crate::{
    auth, auth::AuthResult,
    course::Course,
    inter,
    pace::{BookCh, Goal, Pace, Source},
    store::Store,
    UnifiedError,
    user::{BaseUser, Role, Student, Teacher, User},
};

#[derive(Deserialize)]
struct ConfigFile {
    auth_db_connect_string: Option<String>,
    data_db_connect_string: Option<String>,
    admin_uname: Option<String>,
    admin_password: Option<String>,
    admin_email: Option<String>,
    sendgrid_auth_string: String,
    host: Option<String>,
    port: Option<u16>,
    templates_dir: Option<String>,
    students_per_teacher: Option<usize>,
    goals_per_student: Option<usize>
}

#[derive(Debug)]
pub struct Cfg {
    pub auth_db_connect_string: String,
    pub data_db_connect_string: String,
    pub default_admin_uname: String,
    pub default_admin_password: String,
    pub default_admin_email: String,
    pub sendgrid_auth_string: String,
    pub addr: SocketAddr,
    pub templates_dir: PathBuf,
    pub students_per_teacher: usize,
    pub goals_per_student: usize,
}

impl std::default::Default for Cfg {
    fn default() -> Self {
        Self {
            auth_db_connect_string: "host=localhost user=camp_test password='camp_test' dbname=camp_auth_test".to_owned(),
            data_db_connect_string: "host=localhost user=camp_test password='camp_test' dbname=camp_store_test".to_owned(),
            default_admin_uname: "root".to_owned(),
            default_admin_password: "toot" .to_owned(),
            default_admin_email: "admin@camp.not.an.address".to_owned(),
            sendgrid_auth_string: "".to_owned(),
            addr: SocketAddr::new(
                "0.0.0.0".parse().unwrap(),
                8001
            ),
            templates_dir: PathBuf::from("templates/"),
            students_per_teacher: 60,
            goals_per_student: 16,
        }
    }
}

impl Cfg {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path = path.as_ref();
        let file_contents = std::fs::read_to_string(path)
            .map_err(|e| format!("Unable to read config file: {}", &e))?;
        let cf: ConfigFile = toml::from_str(&file_contents)
            .map_err(|e| format!("Unable to deserialize config file: {}", &e))?;
        
        let mut c = Self::default();
        c.sendgrid_auth_string = cf.sendgrid_auth_string;

        if let Some(s) = cf.auth_db_connect_string {
            c.auth_db_connect_string = s;
        }
        if let Some(s) = cf.data_db_connect_string {
            c.data_db_connect_string = s;
        }
        if let Some(s) = cf.admin_uname {
            c.default_admin_uname = s;
        }
        if let Some(s) = cf.admin_password {
            c.default_admin_password = s;
        }
        if let Some(s) = cf.admin_email {
            c.default_admin_email = s;
        }
        if let Some(s) = cf.host {
            c.addr.set_ip(
                s.parse().map_err(|e| format!(
                    "Error parsing {:?} as IP address: {}",
                    &s, &e
                ))?
            );
        }
        if let Some(n) = cf.port {
            c.addr.set_port(n);
        }
        if let Some(s) = cf.templates_dir {
            c.templates_dir = PathBuf::from(&s);
        }

        Ok(c)
    }
}

/**
This guy will haul around some global variables and be passed in an
`axum::Extension` to the handlers who need him.
*/
pub struct Glob {
    auth: Arc<RwLock<auth::Db>>,
    data: Arc<RwLock<Store>>,
    pub sendgrid_auth: String,
    pub calendar: Vec<Date>,
    pub dates: HashMap<String, Date>,
    pub courses: HashMap<i64, Course>,
    pub course_syms: HashMap<String, i64>,
    pub users: HashMap<String, User>,
    pub addr: SocketAddr,
    pub goals_per_student: usize,
    pub students_per_teacher: usize,
}

impl<'a> Glob {
    pub fn auth(&self) -> Arc<RwLock<auth::Db>> { self.auth.clone() }
    pub fn data(&self) -> Arc<RwLock<Store>>    { self.data.clone() }

    /// Retrieve all `User` data from the database and replace the contents
    /// of the current `.users` map with it.
    pub async fn refresh_users(&mut self) -> Result<(), String> {
        log::trace!("Glob::refresh_users() called.");
        let new_users = self.data.read().await.get_users().await
            .map_err(|e| format!("Error retrieving users from Data DB: {}", &e))?;
        self.users = new_users;
        Ok(())
    }

    /// Retrieve all `Course` data from the database and replace the contents
    /// of the current `.courses` map with it.
    pub async fn refresh_courses(&mut self) -> Result<(), String> {
        log::trace!("Glob::refresh_courses() called.");
        let new_courses = self.data.read().await.get_courses().await
            .map_err(|e| format!("Error retrieving course information from Data DB: {}", &e))?;
        self.courses = new_courses;
        let new_sym_map: HashMap<String, i64> = self.courses.iter()
            .map(|(id, crs)| (crs.sym.clone(), *id))
            .collect();
        self.course_syms = new_sym_map;
        Ok(())
    }

    pub async fn refresh_calendar(&mut self) -> Result<(), String> {
        log::trace!("Glob::refresh_calendar() called.");
        let new_dates = self.data.read().await.get_calendar().await
            .map_err(|e| format!("Error retrieving calendar dates from Data DB: {}", &e))?;
        self.calendar = new_dates;
        self.calendar.sort();
        Ok(())
    }

    pub async fn refresh_dates(&mut self) -> Result<(), String> {
        log::trace!("Glob::refresh_dates() called.");
        let new_dates = self.data.read().await.get_dates().await
            .map_err(|e| format!("Error retrieving special dates from Data DB: {}", &e))?;
        self.dates = new_dates;
        Ok(())
    }

    pub fn course_by_sym(&self, sym: &str) -> Option<&Course> {
        match self.course_syms.get(sym) {
            Some(id) => self.courses.get(id),
            None => None,
        }
    }

    /// Insert the given user into both the auth and the data databases.
    /// 
    /// This takes advantage of the fact that it's necessary to insert into
    /// the data DB and get back a salt string before the user info can be
    /// inserted into the auth DB.
    /// 
    /// XXX TODO XXX
    /// 
    ///   * Generate random passwords upon insertion.
    /// 
    pub async fn insert_user(&self, u: &User) -> Result<(), UnifiedError> {
        log::trace!("Glob::insert_user( {:?} ) called.", u);

        let data = self.data.read().await;
        let mut client = data.connect().await?;
        let t = client.transaction().await?;

        let salt = match u {
            User::Admin(base) => {
                data.insert_admin(&t, &base.uname, &base.email).await?
            },
            User::Boss(base) => {
                data.insert_boss(&t, &base.uname, &base.email).await?
            },
            User::Teacher(teach) => {
                data.insert_teacher(
                    &t,
                    &teach.base.uname,
                    &teach.base.email,
                    &teach.name
                ).await?
            }
            User::Student(s) => {
                let mut studs = vec![s.clone()];
                data.insert_students(&t, &mut studs).await?;
                // .unwrap()ping is fine here, because we just ensured `studs`
                // was a vector of length exactly 1.
                studs.pop().unwrap().base.salt
            }
        };

        {
            let auth = self.auth.read().await;
            let mut auth_client = auth.connect().await?;
            let auth_t = auth_client.transaction().await?;
            auth.add_user(&auth_t, u.uname(), "new_password", &salt,).await?;
            auth_t.commit().await?;
        }

        if let Err(e) = t.commit().await {
            return Err(format!(
                "Unable to commit transaction: {}\nWarning! Auth DB maybe out of sync with Data DB.", &e
            ))?;
        }

        Ok(())
    }

    pub async fn upload_students(&self, csv_data: &str) -> Result<(), UnifiedError> {
        log::trace!(
            "Glob::upload_students( [ {} bytes of CSV body ] ) called.",
            &csv_data.len()
        );

        let mut reader = Cursor::new(csv_data);
        let mut students = Student::vec_from_csv_reader(&mut reader)?;
        {
            let mut not_teachers: Vec<(&str, &str, &str)> = Vec::new();
            for s in students.iter() {
                if let Some(User::Teacher(_)) = self.users.get(&s.teacher) {
                    /* This is the happy path. */
                } else {
                    not_teachers.push((&s.teacher, &s.last, &s.rest));
                }
            }

            if !not_teachers.is_empty() {
                let mut estr = String::from(
                    "You have assigned students to the following unames who are not teachers:\n"
                );
                for (t, last, rest) in not_teachers.iter() {
                    writeln!(&mut estr, "{} (assigned to {}, {})", t, last, rest)
                        .map_err(|e| format!(
                            "Error generating error message: {}\n(Task failed successfully, lol.)", &e
                        ))?;
                }
                return Err(UnifiedError::String(estr));
            }
        }

        let data = self.data.read().await;
        let mut data_client = data.connect().await?;
        let data_t = data_client.transaction().await?;

        let n_studs = data.insert_students(&data_t, &mut students).await?;
        log::trace!("Inserted {} Students into store.", &n_studs);

        let new_password = "this is a new password".to_owned();
        let mut uname_refs: Vec<&str> = Vec::with_capacity(students.len());
        let mut pword_refs: Vec<&str> = Vec::with_capacity(students.len());
        let mut salt_refs:  Vec<&str> = Vec::with_capacity(students.len());
        for s in students.iter() {
            uname_refs.push(&s.base.uname);
            pword_refs.push(&new_password);
            salt_refs.push(&s.base.salt);
        }

        {
            let auth = self.auth.read().await;
            let mut auth_client = auth.connect().await?;
            let auth_t = auth_client.transaction().await?;

            auth.add_users(
                &auth_t, &uname_refs, &pword_refs, &salt_refs
            ).await?;

            auth_t.commit().await?;
        }

        if let Err(e) = data_t.commit().await {
            return Err(format!(
                "Unable to commit transaction: {}\nWarning! Auth DB maybe out of sync with Data DB.", &e
            ))?;
        }

        Ok(())
    }

    pub async fn update_user(&self, u: &User) -> Result<(), UnifiedError> {
        log::trace!("Glob::update_user( {:?} ) called.", u);

        let data = self.data.read().await;
        let mut client = data.connect().await?;
        let t = client.transaction().await?;

        match u {
            User::Admin(_) => {
                data.update_admin(&t, u.uname(), u.email()).await?;
            },
            User::Boss(_) => {
                data.update_boss(&t, u.uname(), u.email()).await?;
            },
            User::Teacher(teach) => {
                data.update_teacher(
                    &t,
                    &teach.base.uname,
                    &teach.base.email,
                    &teach.name
                ).await?;
            },
            User::Student(s) => {
                /*  Here we have to replace several of the fields of `s` from
                    the value stored in `self.users` because the "Admin" user
                    doesn't have access to them, and the values passed from the
                    Admin page will not be correct. */
                let old_u = match self.users.get(&s.base.uname) {
                    Some(ou) => match ou {
                        User::Student(ous) => ous,
                        x => {
                            return Err(format!(
                                "{:?} is not a Student ({}).",
                                &s.base.uname, &x.role()
                            ))?;
                        },
                    },
                    None => { return Err(format!(
                        "{:?} is not a User in the database.", &s.base.uname
                    ))?; },
                };
                let mut s = s.clone();
                s.fall_exam   = old_u.fall_exam.clone();
                s.spring_exam = old_u.spring_exam.clone();
                s.fall_exam_fraction   = old_u.fall_exam_fraction;
                s.spring_exam_fraction = old_u.spring_exam_fraction;
                s.fall_notices   = old_u.fall_notices;
                s.spring_notices = old_u.spring_notices;

                data.update_student(&t, &s).await?;
            },
        }

        t.commit().await?;

        Ok(())
    }

    pub async fn delete_user(&self, uname: &str) -> Result<(), UnifiedError> {
        log::trace!("Glob::delete_user( {:?} ) called.", uname);

        {
            let u = match self.users.get(uname) {
                None => {
                    return Err(UnifiedError::String(format!("No User {:?}.", uname)));
                },
                Some(u) => u,
            };

            if u.role() == Role::Teacher {
                let studs = self.get_students_by_teacher(u.uname());
                if !studs.is_empty() {
                    let mut estr = format!(
                        "The following Students are still assigned to Teacher {:?}\n",
                        u.uname()
                    );
                    for kid in studs.iter() {
                        estr.push_str(kid.uname());
                        estr.push('\n');
                    }
                    return Err(UnifiedError::String(estr));
                }
            }
        }

        let data = self.data.read().await;
        let mut data_client = data.connect().await?;
        let data_t = data_client.transaction().await?;

        data.delete_user(&data_t, uname).await?;
        {
            let auth = self.auth.read().await;
            let mut auth_client = auth.connect().await?;
            let auth_t = auth_client.transaction().await?;
            auth.delete_users(&auth_t, &[uname]).await?;
            auth_t.commit().await?;
        }

        if let Err(e) = data_t.commit().await {
            return Err(format!(
                "Unable to commit transaction: {}\nWarning! Auth DB maybe out of sync with Data DB.", &e
            ))?;
        }

        Ok(())
    }

    pub async fn update_password(
        &self,
        uname: &str,
        new_password: &str
    ) -> Result<(), UnifiedError> {
        log::trace!("Glob::update_password( {:?}, ... ) called.", uname);

        let u = self.users.get(uname).ok_or_else(|| format!(
            "There is no user with uname {:?}.", uname
        ))?;

        self.auth.read().await.set_password(uname, new_password, u.salt()).await?;
        Ok(())
    }

    pub fn get_students_by_teacher(
        &'a self,
        teacher_uname: &'_ str
    ) -> Vec<&'a User> {
        log::trace!("Glob::get_students_by_teacher( {:?} ) called.", teacher_uname);

        let mut stud_refs: Vec<&User> = Vec::new();
        for (_, u) in self.users.iter() {
            if let User::Student(ref s) = u {
                if &s.teacher == teacher_uname {
                    stud_refs.push(u);
                }
            }
        }

        return stud_refs;
    }

    pub async fn delete_chapter(
        &self,
        id: i64
    ) -> Result<(), UnifiedError> {
        log::trace!("Glob::delete_chapter( {:?} ) called.", &id);

        let data = self.data();
        let data_read = data.read().await;
        let mut client = data_read.connect().await?;
        let t = client.transaction().await?;

        let rows = t.query(
            "WITH ch_data AS (
                SELECT
                    chapters.course AS crs_n,
                    chapters.sequence AS ch_n,
                    courses.sym AS sym,
                    courses.title AS crs,
                    courses.book AS book,
                    chapters.title AS chp
                FROM chapters
                INNER JOIN courses ON
                    courses.id = chapters.course
                WHERE chapters.id = $1
            )
            SELECT
                ch_data.crs_n, goals.sym, ch_data.ch_n, goals.uname,
                ch_data.crs, ch_data.chp, ch_data.book
            FROM goals INNER JOIN ch_data ON goals.sym = ch_data.sym",
            &[&id]
        ).await?;

        if !rows.is_empty() {
            let row = &rows[0];
            log::debug!("{:?}", row);
            let sym: String = row.try_get("sym")?;
            let seq: i16 = row.try_get("seq")?;
            let title: String = row.try_get("crs")?;
            let chapter: String = row.try_get("chp")?;
            let book: String = match row.try_get("book")? {
                Some(s) => s,
                None => String::from("[ no listed book ]"),
            };
            let mut unames: HashSet<String> = HashSet::with_capacity(rows.len());
            for row in rows.iter() {
                let uname: String = row.try_get("uname")?;
                unames.insert(uname);
            }

            let mut estr = format!(
                "Chapter ({:?}, {:?}) ({}, {} from {}) cannot be deleted because the following users have that Chapter as a Goal:\n",
                &sym, &seq, &title, &chapter, &book
            );
            for uname in unames.iter() {
                if let Some(User::Student(ref s)) = self.users.get(uname.as_str()) {
                    writeln!(&mut estr, "{} ({}, {})", uname, &s.last, &s.rest)
                        .map_err(|e| format!("Error generating error message: {}", &e))?;
                }
            }

            return Err(estr.into());
        }

        t.execute("DELETE FROM chapters WHERE id = $1", &[&id]).await?;

        t.commit().await.map_err(|e| format!("Error commiting transaction to delete Chapter w/id {}", &id))?;

        Ok(())
    }

    pub async fn delete_course(
        &self,
        sym: &str
    ) -> Result<(usize, usize), UnifiedError> {
        log::trace!("Glob::delete_course( {:?} ) called.", sym);

        let data = self.data();
        let data_read = data.read().await;
        let mut client = data_read.connect().await?;
        let t = client.transaction().await?;

        let rows = t.query(
            "SELECT DISTINCT uname FROM goals WHERE sym = $1", &[&sym]
        ).await?;

        if !rows.is_empty() {
            let crs = self.course_by_sym(sym).ok_or_else(|| format!(
                "There is no course with symbol {:?}.", sym
            ))?;
            let mut estr = format!(
                "The Course {:?} ({} from {}) cannot be deleted because the following users have Goals from that Course:\n",
                sym, &crs.title, &crs.book
            );
            for row in rows.iter() {
                let uname: &str = row.try_get("uname")?;
                if let Some(User::Student(ref s)) = self.users.get(uname) {
                    writeln!(&mut estr, "{} ({}, {})", uname, &s.last, &s.rest)
                        .map_err(|e| format!("Error generating error message: {}", &e))?;
                }
            }

            return Err(estr.into());
        }

        let tup = data_read.delete_course(&t, sym).await?;

        match t.commit().await {
            Ok(_) => Ok(tup),
            Err(e) => Err(e.into())
        }
    }

    pub async fn insert_goals(
        &self,
        goals: &[Goal]
    ) -> Result<usize, UnifiedError> {
        log::trace!(
            "Glob::insert_goals( [ {} Goals ] ) called.", &goals.len()
        );

        // First we want to check the unames courses on all the goals and
        // ensure those exist before we start trying to insert. This will
        // allow us to produce a better error message for the user.
        {
            let mut unk_users: HashSet<String> = HashSet::new();
            let mut unk_courses: HashSet<String> = HashSet::new();
            for g in goals.iter() {
                match self.users.get(&g.uname) {
                    Some(User::Student(_)) => { /* This is what we hope is true! */ },
                    _ => { unk_users.insert(g.uname.clone()); }
                }
                match g.source {
                    Source::Book(ref bch) => {
                        if let None = self.course_syms.get(&bch.sym) {
                            unk_courses.insert(bch.sym.clone());
                        }
                    },
                    _ => { 
                        return Err("Custom Courses not yet supported.".to_owned().into()); 
                    },
                }
            }

            if unk_users.len() > 0 || unk_courses.len() > 0 {
                let mut estr = String::new();
                if unk_users.len() > 0 {
                    writeln!(
                        &mut estr,
                        "The following user names do not belong to known students:"
                    ).map_err(|e| format!("Error preparing error message: {}!!!", &e))?;
                    for uname in unk_users.iter() {
                        writeln!(&mut estr, "{}", uname).map_err(|e| format!(
                            "Error preparing error message: {}!!!", &e
                        ))?;
                    }
                }
                if unk_courses.len() > 0 {
                    writeln!(
                        &mut estr,
                        "The following symbols do not belong to known courses:"
                    ).map_err(|e| format!("Error preparing error message: {}!!!", &e))?;
                    for sym in unk_courses.iter() {
                        writeln!(&mut estr, "{}", sym).map_err(|e| format!(
                            "Error preparing error message: {}!!!", &e
                        ))?;
                    }
                }

                return Err(estr.into());
            }
        }

        let n_inserted = self.data.read().await.insert_goals(goals).await?;
        Ok(n_inserted)
    }

    pub async fn get_pace_by_student(
        &self,
        uname: &str
    ) -> Result<Pace, UnifiedError> {
        log::trace!("Glob::get_pace_by_student( {:?} ) called.", uname);

        let stud = match self.users.get(uname) {
            Some(User::Student(s)) => s.clone(),
            _ => {
                return Err(format!(
                    "{:?} is not a Student in the database.", uname
                ).into());
            }
        };
        let teach = match self.users.get(&stud.teacher) {
            Some(User::Teacher(t)) => t.clone(),
            _ => {
                return Err(format!(
                    "{:?} has teacher {:?}, but {:?} is not a teacher.",
                    &stud.base.uname, &stud.teacher, &stud.teacher
                ).into());
            }
        };

        let goals = self.data.read().await.get_goals_by_student(uname).await?;

        let p = Pace::new(stud, teach, goals, &self)?;
        Ok(p)
    }

    pub async fn get_paces_by_teacher(
        &self,
        tuname: &str
    ) -> Result<Vec<Pace>, UnifiedError> {
        log::trace!("Glob::get_paces_by_teacher( {:?} ) called.", tuname);

        let teach = match self.users.get(tuname) {
            Some(User::Teacher(t)) => t.clone(),
            _ => {
                return Err(format!(
                    "{:?} is not a Teacher in the database.", tuname
                ).into());
            },
        };

        let students = self.get_students_by_teacher(tuname);

        let mut goals = self.data.read().await.get_goals_by_teacher(tuname).await?;

        let mut goal_map: HashMap<String, Vec<Goal>> =
            HashMap::with_capacity(students.len());
        
        for g in goals.drain(..) {
            if let Some(v) = goal_map.get_mut(&g.uname) {
                (*v).push(g)
            } else {
                let uname = g.uname.clone();
                let v = vec![g];
                goal_map.insert(uname, v);
            }
        }

        for s in students {
            if goal_map.get(s.uname()).is_none() {
                goal_map.insert(s.uname().to_string(), vec![]);
            }
        }

        let mut cals: Vec<Pace> = Vec::with_capacity(goal_map.len());
        for (uname, v) in goal_map.drain() {
            let s = match self.users.get(&uname) {
                Some(User::Student(s)) => s.clone(),
                x => {
                    log::error!(
                        "Vector of goals belonging to {:?}, but this uname belongs not to a Student in the database ({:?}).",
                        &uname, &x
                    );
                    continue;
                },
            };

            let p = match Pace::new(s, teach.clone(), v, &self) {
                Ok(p) => p,
                Err(e) => {
                    log::error!("Error generating Pace calendar for {:?}: {}", &uname, &e);
                    continue;
                },
            };

            cals.push(p);
        }

        Ok(cals)
    }
}

async fn insert_default_admin_into_data_db(
    cfg: &Cfg,
    data: &Store,
) -> Result<User, UnifiedError> {
    {
        let mut client = data.connect().await?;
        let t = client.transaction().await?;
        data.insert_admin(
            &t,
            &cfg.default_admin_uname,
            &cfg.default_admin_password
        ).await?;
        t.commit().await?;
    }

    match data.get_user_by_uname(&cfg.default_admin_uname).await {
        Err(e) => Err(format!(
            "Error attempting to retrieve newly-inserted default Admin user: {}", &e
        ))?,
        Ok(None) => Err(format!(
            "Newly-inserted Admin still not present in Data DB for some reason."
        ))?,
        Ok(Some(u)) => Ok(u)
    }
}

async fn insert_default_admin_into_auth_db(
    cfg: &Cfg,
    u: &User,
    auth: &auth::Db
) -> Result<(), UnifiedError> {
    let mut client = auth.connect().await?;
    let t = client.transaction().await?;
    auth.add_user(
        &t,
        u.uname(),
        &cfg.default_admin_password,
        u.salt()
    ).await?;
    t.commit().await?;

    Ok(())
}

/// Loads system configuration and ensures all appropriate database tables
/// exist.
///
/// Also assures existence of default admin.
pub async fn load_configuration<P: AsRef<Path>>(path: P)
-> Result<Glob, UnifiedError> {
    let cfg = Cfg::from_file(path.as_ref())?;
    log::info!("Configuration file read:\n{:#?}", &cfg);

    log::trace!("Checking state of auth DB...");
    let auth_db = auth::Db::new(cfg.auth_db_connect_string.clone());
    if let Err(e) = auth_db.ensure_db_schema().await {
        let estr = format!("Unable to ensure state of auth DB: {}", &e);
        return Err(estr)?;
    }
    log::trace!("...auth DB okay.");
    let n_old_keys = auth_db.cull_old_keys().await?;
    log::info!("Removed {} expired keys from Auth DB.", &n_old_keys);

    log::trace!("Checking state of data DB...");
    let data_db = Store::new(cfg.data_db_connect_string.clone());
    if let Err(e) = data_db.ensure_db_schema().await {
        let estr = format!("Unable to ensure state of data DB: {}", &e);
        return Err(estr)?;
    }
    log::trace!("...data DB okay.");

    log::trace!("Checking existence of default Admin in data DB...");
    let default_admin = match data_db.get_user_by_uname(
        &cfg.default_admin_uname
    ).await {
        Err(e) => {
            let estr = format!(
                "Error attempting to check existence of default Admin ({}) in data DB: {}",
                &cfg.default_admin_uname, &e
            );
            return Err(estr)?;
        },
        Ok(None) => {
            log::info!(
                "Default Admin ({}) doesn't exist in data DB; inserting.",
                &cfg.default_admin_uname
            );

            let u = insert_default_admin_into_data_db(&cfg, &data_db).await
                .map_err(|e| format!(
                    "Error attempting to insert default Admin user into Data DB: {}", &e
                ))?;
            u
        },
        Ok(Some(u)) => u,
    };
    log::trace!("Default admin OK in data DB.");

    log::trace!("Checking existence of default Admin in auth DB...");
    match auth_db.check_password(
        default_admin.uname(),
        &cfg.default_admin_password,
        default_admin.salt(),
    ).await {
        Err(e) => {
            let estr = format!("Error checking existence of default Admin in auth DB: {}", &e);
            return Err(estr)?;
        },
        Ok(AuthResult::BadPassword) => {
            log::warn!("Default Admin ({}) not using default password.", default_admin.uname());
        },
        Ok(AuthResult::NoSuchUser) => {
            log::info!("Default Admin ({}) doesn't exist in auth DB; inserting.", default_admin.uname());
            insert_default_admin_into_auth_db(&cfg, &default_admin, &auth_db).await
                .map_err(|e| format!(
                    "Error attempting to insert default Admin into Auth DB: {}", &e
                ))?;
            log::trace!("Default Admin inserted into auth DB.");
        },
        Ok(AuthResult::Ok) => {
            log::trace!("Default Admin password check OK.");
        },
        Ok(x) => {
            let estr = format!("Default Admin password check resulted in {:?}, which just doesn't make sense.", &x);
            return Err(estr)?;
        },
    }
    log::trace!("Default Admin OK in auth DB.");

    let mut glob = Glob {
        auth: Arc::new(RwLock::new(auth_db)),
        data: Arc::new(RwLock::new(data_db)),
        sendgrid_auth: cfg.sendgrid_auth_string,
        dates: HashMap::new(),
        calendar: Vec::new(),
        courses: HashMap::new(),
        course_syms: HashMap::new(),
        users: HashMap::new(),
        addr: cfg.addr,
        goals_per_student: cfg.goals_per_student,
        students_per_teacher: cfg.students_per_teacher,
    };

    glob.refresh_courses().await?;
    log::info!("Retrieved {} courses from data DB.", glob.courses.len());
    
    glob.refresh_users().await?;
    log::info!("Retrieved {} users from data DB.", glob.users.len());

    glob.refresh_calendar().await?;
    log::info!("Retrieved {} instructional days from data DB.", glob.calendar.len());

    glob.refresh_dates().await?;
    log::info!("Retrieved {} special dates from data DB.", glob.dates.len());
    log::debug!("special dates:\n{:#?}\n", &glob.dates);

    inter::init(&cfg.templates_dir)?;

    Ok(glob)
}

#[cfg(test)]
mod tests {
    use crate::*;
    use crate::pace::{Pace, Source};
    use crate::tests::ensure_logging;

    use serial_test::serial;

    static CONFIG: &str = "fakeprod_data/config.toml";

    #[tokio::test]
    #[serial]
    async fn get_one_pace() -> Result<(), UnifiedError> {
        ensure_logging();

        let glob = config::load_configuration(CONFIG).await?;

        let p = glob.get_pace_by_student("eparker").await?;
        println!("{:#?}", &p);

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn autopace() -> Result<(), UnifiedError> {
        ensure_logging();

        let glob = config::load_configuration(CONFIG).await?;

        let mut p: Pace = glob.get_pace_by_student("wholt").await?;
        p.autopace(&glob.calendar)?;
        for g in p.goals.iter() {
            let source = match &g.source {
                Source::Book(src) => src,
                _ => panic!("No custom chapters!"),
            };

            let crs = glob.course_by_sym(&source.sym).unwrap();
            let chp = crs.chapter(source.seq).unwrap();
            let datestr = match g.due {
                None => "None".to_string(),
                Some(d) => format!("{}", &d),
            };
            println!(
                "{}: {} {} {:?}",
                &g.id, &crs.title, &chp.title, &datestr
            );
        }

        Ok(())
    }
}