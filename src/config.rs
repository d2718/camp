/*!
Structs to hold configuration data and global variables.
*/
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;

use handlebars::Handlebars;
use serde::Deserialize;

use crate::{
    auth, auth::AuthResult,
    course::Course,
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
}

#[derive(Debug)]
pub struct Cfg {
    pub auth_db_connect_string: String,
    pub data_db_connect_string: String,
    pub default_admin_uname: String,
    pub default_admin_password: String,
    pub default_admin_email: String,
    pub addr: SocketAddr,
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

        Ok(c)
    }
}

/**
This guy will haul around some global variables and be passed in an
`axum::Extension` to the handlers who need him.
*/
#[derive(Debug)]
pub struct Glob<'a> {
    pub auth_db_connect_string: String,
    pub data_db_connect_string: String,
    pub courses: HashMap<i64, Course>,
    pub users: HashMap<String, User>,
    pub addr: SocketAddr,
    pub templates: Handlebars<'a>,
}

/// Loads system configuration and ensures all appropriate database tables
/// exist.
///
/// Also assures existence of default admin.
pub async fn load_configuration<P: AsRef<Path>>(path: P) -> Result<Glob<'static>, String> {
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

    log::trace!("Retrieving courses from data DB.");
    let courses = data_db.get_courses().await
        .map_err(|e| format!("Error retrieving courses from data DB: {}", &e))?;
    log::info!("Retrieved {} courses from data DB.", &courses.len());
    
    log::trace!("Retrieving users from data DB.");
    let users = data_db.get_users().await
        .map_err(|e| format!("Error retrieving users from data DB: {}", &e))?;
    log::info!("Retrieved {} users from data DB.", &users.len());

    let mut templates = Handlebars::new();
    templates.register_templates_directory(".html", "templates/")
        .map_err(|e| format!("Error registering template directory: {}", &e))?;

    let glob = Glob {
        auth_db_connect_string: cfg.auth_db_connect_string,
        data_db_connect_string: cfg.data_db_connect_string,
        courses,
        users,
        addr: cfg.addr,
        templates,
    };

    Ok(glob)
}