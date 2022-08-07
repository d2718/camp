/*!
Structs to hold configuration data and global variables.
*/
use std::collections::HashMap;
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
    pub async fn insert_user(&self, u: &User) -> Result<(), String> {
        log::trace!("Glob::insert_user( {:?} ) called.", u);

        let salt = match u {
            User::Admin(base) => {
                let data = self.data.read().await;
                data.insert_admin(&base.uname, &base.email).await?
            },
            User::Boss(base) => {
                let data = self.data.read().await;
                data.insert_boss(&base.uname, &base.email).await?
            },
            User::Teacher(t) => {
                let data = self.data.read().await;
                data.insert_teacher(
                    &t.base.uname,
                    &t.base.email,
                    &t.name
                ).await?
            }
            User::Student(s) => {
                let data = self.data.read().await;
                let mut studs = vec![s.clone()];
                data.insert_students(&mut studs).await?;
                // .unwrap()ping is fine here, because we just ensured `studs`
                // was a vector of length exactly 1.
                studs.pop().unwrap().base.salt
            }
        };

        {
            let auth = self.auth.read().await;
            if let Err(e) = auth.add_user(
                u.uname(),
                "new_password",
                &salt,
            ).await {
                self.data.read().await.delete_user(u.uname()).await?;
                return Err(format!("Unable to insert new user: {}", &e));
            }
        }

        Ok(())
    }

    pub async fn update_user(&self, u: &User) -> Result<(), String> {
        log::trace!("Glob::update_user( {:?} ) called.", u);

        let data = self.data.read().await;
        match u {
            User::Admin(_) => {
                data.update_admin(u.uname(), u.email()).await?;
            },
            User::Boss(_) => {
                data.update_boss(u.uname(), u.email()).await?;
            },
            User::Teacher(t) => {
                data.update_teacher(&t.base.uname, &t.base.email, &t.name).await?;
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
                            ));
                        },
                    },
                    None => { return Err(format!(
                        "{:?} is not a User in the database.", &s.base.uname
                    )); },
                };
                let mut s = s.clone();
                s.fall_exam   = old_u.fall_exam.clone();
                s.spring_exam = old_u.spring_exam.clone();
                s.fall_exam_fraction   = old_u.fall_exam_fraction;
                s.spring_exam_fraction = old_u.spring_exam_fraction;
                s.fall_notices   = old_u.fall_notices;
                s.spring_notices = old_u.spring_notices;

                data.update_student(&s).await?;
            },
        }

        Ok(())
    }

    pub async fn delete_user(&self, uname: &str) -> Result<(), String> {
        log::trace!("Glob::delete_user( {:?} ) called.", uname);

        {
            let data = self.data.read().await;
            data.delete_user(uname).await?;
        }
        {
            let auth = self.auth.read().await;
            auth.delete_users(&[uname]).await?;
        }

        Ok(())
    }
}

/// Loads system configuration and ensures all appropriate database tables
/// exist.
///
/// Also assures existence of default admin.
pub async fn load_configuration<P: AsRef<Path>>(path: P) -> Result<Glob, String> {
    let cfg = Cfg::from_file(path.as_ref())?;
    log::info!("Configuration file read:\n{:#?}", &cfg);

    log::trace!("Checking state of auth DB...");
    let auth_db = auth::Db::new(cfg.auth_db_connect_string.clone());
    if let Err(e) = auth_db.ensure_db_schema().await {
        let estr = format!("Unable to ensure state of auth DB: {}", &e);
        return Err(estr);
    }
    log::trace!("...auth DB okay.");

    log::trace!("Checking state of data DB...");
    let data_db = Store::new(cfg.data_db_connect_string.clone());
    if let Err(e) = data_db.ensure_db_schema().await {
        let estr = format!("Unable to ensure state of data DB: {}", &e);
        return Err(estr);
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
            return Err(estr);
        },
        Ok(None) => {
            log::info!(
                "Default Admin ({}) doesn't exist in data DB; inserting.",
                &cfg.default_admin_uname
            );
            if let Err(e) = data_db.insert_admin(
                &cfg.default_admin_uname,
                &cfg.default_admin_email
            ).await {
                let estr = format!(
                    "Error inserting default Admin into data DB: {}",
                    &e
                );
                return Err(estr);
            }
            match data_db.get_user_by_uname(&cfg.default_admin_uname).await {
                Err(e) => {
                    let estr = format!("Error attempting to retrieve newly-inserted default Admin: {}", &e);
                    return Err(estr);
                },
                Ok(None) => {
                    let estr = format!("Newly-inserted default Admin still not there for some reason.");
                    return Err(estr);
                },
                Ok(Some(u)) => u,
            }
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
            return Err(estr);
        },
        Ok(AuthResult::BadPassword) => {
            log::warn!("Default Admin ({}) not using default password.", default_admin.uname());
        },
        Ok(AuthResult::NoSuchUser) => {
            log::info!("Default Admin ({}) doesn't exist in auth DB; inserting.", default_admin.uname());
            if let Err(e) = auth_db.add_user(
                default_admin.uname(),
                &cfg.default_admin_password,
                default_admin.salt()
            ).await {
                let estr = format!("Error inserting default Admin into auth DB: {}", &e);
                return Err(estr);
            };
            log::trace!("Default Admin inserted into auth DB.");
        },
        Ok(AuthResult::Ok) => {
            log::trace!("Default Admin password check OK.");
        },
        Ok(x) => {
            let estr = format!("Default Admin password check resulted in {:?}, which just doesn't make sense.", &x);
            return Err(estr);
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