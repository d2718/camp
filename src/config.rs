/*!
Structs to hold configuration data and global variables.
*/
use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::io::Cursor;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use handlebars::Handlebars;
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::{
    auth, auth::AuthResult,
    course::Course,
    inter,
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
    host: Option<String>,
    port: Option<u16>,
    templates_dir: Option<String>
}

#[derive(Debug)]
pub struct Cfg {
    pub auth_db_connect_string: String,
    pub data_db_connect_string: String,
    pub default_admin_uname: String,
    pub default_admin_password: String,
    pub default_admin_email: String,
    pub addr: SocketAddr,
    pub templates_dir: PathBuf,
}

impl std::default::Default for Cfg {
    fn default() -> Self {
        Self {
            auth_db_connect_string: "host=localhost user=camp_test password='camp_test' dbname=camp_auth_test".to_owned(),
            data_db_connect_string: "host=localhost user=camp_test password='camp_test' dbname=camp_store_test".to_owned(),
            default_admin_uname: "root".to_owned(),
            default_admin_password: "toot" .to_owned(),
            default_admin_email: "admin@camp.not.an.address".to_owned(),
            addr: SocketAddr::new(
                "0.0.0.0".parse().unwrap(),
                8001
            ),
            templates_dir: PathBuf::from("templates/"),
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
    pub courses: HashMap<i64, Course>,
    pub users: HashMap<String, User>,
    pub addr: SocketAddr,
}

impl Glob {
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
        Ok(())
    }

    /// Insert the given user into both the auth and the data databases.
    /// 
    /// This takes advantage of the fact that it's necessary to insert into
    /// the data DB and get back a salt string before the user info can be
    /// inserted into the auth DB.
    /// 
    /// XXX TODO XXX
    /// 
    ///   * Implement this for `User::Teacher` and `User::Student`.
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
        courses: HashMap::new(),
        users: HashMap::new(),
        addr: cfg.addr,
    };

    glob.refresh_courses().await?;
    log::info!("Retrieved {} courses from data DB.", glob.courses.len());
    
    glob.refresh_users().await?;
    log::info!("Retrieved {} users from data DB.", glob.users.len());

    inter::init(&cfg.templates_dir)?;

    Ok(glob)
}