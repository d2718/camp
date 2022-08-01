/*!
Authentication database connection and methods.

This struct is meant to interface with a database with the following
schema:

```sql
CREATE TABLE users (
    uname TEXT PRIMARY KEY,
    hash  TEXT
);

CREATE TABLE keys (
    key       TEXT,
    uname     TEXT REFERENCES users,
    last_used TIMESTAMP
);
```

Additionally, each `uname` should have a short `salt` string associated with
it (stored separately somewhere) for use in password hashing.
*/
use blake3::{Hash, Hasher};
use tokio_postgres::{Client, NoTls, Statement, types::Type};
use rand::{Rng, distributions};

const DEFAULT_KEY_LENGTH: usize = 32;
const DEFAULT_KEY_CHARS: &str =
"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{}|/?;:,.<>~";
const DEFAULT_KEY_LIFE_SECONDS: u64 = 20 * 60;  // 20 minutes

static SCHEMA_TEST: &[&str] = &[
    "SELECT FROM information_schema.tables WHERE table_name = 'users'",
    "SELECT FROM information_schema.tables WHERE table_name = 'keys'",
];

static SCHEMA: &[&str] = &[
    "CREATE TABLE users (
        uname TEXT PRIMARY KEY,
        hash  TEXT
    )",
    
    "CREATE TABLE keys (
        key TEXT,
        uname TEXT REFERENCES users,
        last_used TIMESTAMP
    )",
];

fn hash_with_salt(pwd: &str, salt: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(pwd.as_bytes());
    hasher.update(salt);
    let hash = hasher.finalize();
    String::from(hash.to_hex().as_str())
}

#[derive(Debug, PartialEq)]
pub struct DbError(String);

impl From<tokio_postgres::error::Error> for DbError {
    fn from(e: tokio_postgres::error::Error) -> DbError {
        use std::fmt::Write;
        
        let mut s = format!("DB: {}", &e);
        if let Some(dbe) = e.as_db_error() {
            write!(&mut s, "; {}", dbe).unwrap();
        }
        DbError(s)
    }
}

impl From<String> for DbError {
    fn from(s: String) -> DbError { DbError(s) }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

/**
Possible results of attempting to authenticate with the database.
*/
#[derive(Debug, PartialEq)]
pub enum AuthResult {
    /// Password or Key authentication successful.
    Ok,
    /// Password successful; issuing key.
    Key(String),
    NoSuchUser,
    BadPassword,
    InvalidKey,
}

pub struct Db {
    connection_string: String,
    key_chars: Vec<char>,
    key_length: usize,
    key_life: String,
}

impl Db {
    pub fn new(connection_string: String) -> Self {
        log::trace!("Db::new( {:?} ) called", &connection_string);
        
        let key_chars: Vec<char> = DEFAULT_KEY_CHARS.chars().collect();
        let key_length = DEFAULT_KEY_LENGTH;
        let key_life = format!("{} seconds", &DEFAULT_KEY_LIFE_SECONDS);
        
        Self {
            connection_string,
            key_chars,
            key_length,
            key_life,
        }
    }
    
    /// Will silently do nothing if `new_chars` is of length zero.
    pub fn set_key_chars(&mut self, new_chars: &str) {
        if new_chars.len() > 0 {
            self.key_chars = new_chars.chars().collect();
        }
    }
    pub fn set_key_length(&mut self, new_length: usize) { self.key_length = new_length; }
    pub fn set_key_life(&mut self, seconds: u64) { self.key_life = format!("{} seconds", &seconds); }
    
    /// Generate a new authentication key based on the current values of
    /// `self.key_chars` and `self.key_length`.
    fn generate_key(&self) -> String {
        // self.key_chars should never be of length 0.
        let dist = distributions::Slice::new(&self.key_chars).unwrap();
        let rng = rand::thread_rng();
        let new_key: String = rng.sample_iter(&dist)
            .take(self.key_length)
            .collect();
        new_key
    }
    
    async fn connect(&self) -> Result<Client, DbError> {
        log::trace!(
            "Db::connect() called w/connection string: {:?}",
            &self.connection_string
        );
        
        match tokio_postgres::connect(&self.connection_string, NoTls).await {
            Ok((client, connection)) => {
                log::trace!("    ...connection successful.");
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        log::error!("Auth DB connection error: {}", &e);
                    } else {
                        log::trace!("tokio connection runtime drops.");
                    }
                });
                Ok(client)
            },
            Err(e) => {
                log::trace!("    ...connection failed: {:?}" ,&e);
                Err(format!("Connection error: {}", &e).into())
            }
        }
    }
    
    pub async fn ensure_db_schema(&self) -> Result<(), DbError> {
        log::trace!("Db::ensure_db_schema() called.");
        let mut client = self.connect().await?;
        let t = client.transaction().await
            .map_err(|e| format!("Auth DB unable to begin transaction: {}", &e))?;
        for (test_stmt, create_stmt) in std::iter::zip(SCHEMA_TEST, SCHEMA) {
            if t.query_opt(test_stmt.to_owned(), &[]).await?.is_none() {
                log::info!("{:?} returned no results.", &test_stmt);
                log::info!("Attempting to insert table.");
                t.execute(create_stmt.to_owned(), &[]).await?;
            }
        }
        
        t.commit().await
            .map_err(|e| format!("Error committing transaction: {}", &e).into())
    }
    
    /**
    Add the specified users to the database.
    
    Will fail with an error if any of the provided `unames` belong to extant
    users.
    */
    pub async fn add_users(
        &self,
        unames: &[&str],
        passwords: &[&str],
        salts: &[&str]
    ) -> Result<u64, DbError> {
        log::trace!(
            "Db::add_users() called with\n    {:?}\n    {:?}\n    {:?}",
            unames, passwords, salts
        );
        
        if unames.len() != passwords.len() {
            log::trace!("unames length doesn't match passwords length.");
            let estr = DbError(format!(
                "Number of unames ({}) and passwords ({}) must match.",
                unames.len(), passwords.len()
            ));
            return Err(estr);
        }
        if passwords.len() != salts.len() {
            log::trace!("passwords length doesn't match salts length.");
            let estr = DbError(format!(
                "Number of passwords ({}) and salts ({}) must match.",
                passwords.len(), salts.len()
            ));
            return Err(estr);
        }
        
        let owned_unames: Vec<String> = unames.iter().map(|s| String::from(*s)).collect();
        
        let hashes: Vec<String> = std::iter::zip(passwords, salts)
            .map(|(pwd, salt)| hash_with_salt(pwd, salt.as_bytes()))
            .collect();
        
        let mut client = self.connect().await?;
        let t = client.transaction().await
            .map_err(|e| format!("Auth DB unable to begin transaction: {}", &e))?;
        
        let preexisting_user_query = t.prepare_typed(
            "SELECT uname FROM users WHERE uname = ANY($1)",
            &[Type::TEXT_ARRAY]
        ).await?;
            
        let preexisting_user_rows = t.query(
            &preexisting_user_query,
            &[&owned_unames]
        ).await.map_err(|e| format!("Error querying for preexisting user names: {}", &e))?;
        if preexisting_user_rows.len() > 0 {
            let preexisting_names: Vec<String> = preexisting_user_rows.iter()
                .map(|r| { 
                    let u: String = r.get("uname");
                    u
                }).collect();
            let estr = format!(
                "Database already contains unames: {:?}",
                &preexisting_names
            );
            return Err(DbError(estr));
        }
        
        let s_add_user = t.prepare_typed(
            "INSERT INTO users (uname, hash) VALUES ($1, $2)",
            &[Type::TEXT, Type::TEXT]
        ).await.map_err(|e|
            format!("Unable to prepare statement to insert new users: {}", &e)
        )?;
        
        let mut n_inserted: u64 = 0;
        for (uname, hash) in std::iter::zip(unames, hashes) {
            match t.execute(&s_add_user, &[&uname, &hash]).await {
                Ok(n) => { n_inserted += n; },
                Err(e) => {
                    log::warn!(
                        "Error inserting (uname, hash) pair ({:?}, {:?}: {}",
                        &uname, &hash, &e
                    );
                },
            }
        }
        match t.commit().await {
            Ok(()) => {
                log::trace!("Inserted {} of {} users.", &n_inserted, &unames.len());
                Ok(n_inserted)
            },
            Err(e) => Err(DbError(format!("Error commiting transaction: {}", &e))),
        }
    }
    
    /// Convenience wrapper around `Db::add_users()` to just add one user.
    pub async fn add_user(
        &self,
        uname: &str,
        password: &str,
        salt: &str,
    ) -> Result<(), DbError> {
        log::trace!(
            "Db::add_user( {:?}, {:?}, {:?} ) called",
            uname, password, salt
        );
        
        match self.add_users(&[uname], &[password], &[salt]).await {
            Err(e) => Err(e),
            Ok(0) => Err(DbError(format!("Failed to add user {:?}", uname))),
            Ok(1) => Ok(()),
            Ok(n) => Err(DbError(format!(
                "Attempt to add 1 user resulted in adding {}; this shouldn't happen.",
                &n
            ))),
        }
    }
    
    pub async fn delete_users(&self, unames: &[&str]) -> Result<u64, DbError> {
        log::trace!("Db::delete_users( {:?} ) called", &unames);
        
        let owned_unames: Vec<String> = unames.iter().map(|s| String::from(*s)).collect();
        
        let mut client = self.connect().await?;
        let t = client.transaction().await?;
        let n_keys = t.execute(
            "DELETE FROM keys WHERE uname = ANY($1)",
            &[&owned_unames]
        ).await?;
        log::trace!("Deleted {} keys.", &n_keys);
        
        let n_users = t.execute(
            "DELETE FROM users WHERE uname = ANY($1)",
            &[&owned_unames]
        ).await?;
        log::trace!("Deleted {} users.", &n_users);
        t.commit().await?;
        
        Ok(n_users)
    }
    
    pub async fn check_password(
        &self,
        uname: &str,
        password: &str,
        salt: &str
    ) -> Result<AuthResult, DbError> {
        log::trace!("Db::check_password( {:?}, {:?}, {:?} ) called.", uname, password, salt);
        
        let current_hash = hash_with_salt(password, salt.as_bytes());
        
        let client = self.connect().await?;
        
        match client.query_opt(
            "SELECT hash FROM users WHERE uname = $1",
            &[&uname]
        ).await {
            Err(e) => {
                let estr = format!("Error querying user {:?}: {}", uname, &e);
                log::error!("{}", &estr);
                Err(DbError(estr))
            },
            Ok(None) => {
                log::trace!("User {:?} doesn't exist.", uname);
                Ok(AuthResult::NoSuchUser)
            },
            Ok(Some(row)) => {
                let stored_hash: String = row.get("hash");
                if stored_hash == current_hash {
                    Ok(AuthResult::Ok)
                } else {
                    Ok(AuthResult::BadPassword)
                }
            },
        }
    }
    
    /**
    Check whether the provided `(uname, password, salt)` combination is valid,
    and issue a new key on success.
    */
    pub async fn check_password_and_issue_key(
        &self,
        uname: &str,
        password: &str,
        salt: &str
    ) -> Result<AuthResult, DbError> {
        log::trace!(
            "Db::check_password_and_issue_key( {:?}, {:?}, {:?} ) called.",
            uname, password, salt
        );
        
        let current_hash = hash_with_salt(password, salt.as_bytes());
        
        let client = self.connect().await?;
        
        match client.query_opt(
            "SELECT hash FROM users WHERE uname = $1",
            &[&uname]
        ).await {
            Err(e) => {
                let estr = format!("Error querying user {:?}: {}", uname, &e);
                log::error!("{}", &estr);
                return Err(DbError(estr));
            },
            Ok(None) => {
                log::trace!("User {:?} doesn't exist.", uname);
                return Ok(AuthResult::NoSuchUser);
            },
            Ok(Some(row)) => {
                let stored_hash: String = row.get("hash");
                if stored_hash != current_hash {
                    return Ok(AuthResult::BadPassword);
                }
            },
        }
        
        let key = self.generate_key();
        if let Err(e) = client.execute(
            "INSERT INTO keys (uname, key, last_used)
            VALUES ($1, $2, CURRENT_TIMESTAMP)",
            &[&uname, &key]
        ).await {
            return Err(e.into());
        }
        
        log::trace!("Returning new key: {:?}", &key);
        Ok(AuthResult::Key(key))
    }
    
    /**
    Checks to see if the provided `key` was issued to the provided `uname`
    and is still valid.
        
    Also updates the key's `last_used` time to the current time on success.
    */
    async fn check_key(
        &self,
        uname: &str,
        key: &str
    ) -> Result<AuthResult, DbError> {
        log::trace!("Db::check_key( {:?}, {:?} ) called.", uname, key);
        
        let client = self.connect().await?;
        let key = match client.query_opt(
            "SELECT key FROM keys
                WHERE uname = $1
                AND key = $2
                AND last_used + ($3 || ' ')::INTERVAL > now()",
            &[&uname, &key, &self.key_life]
        ).await? {
            None => { return Ok(AuthResult::InvalidKey); },
            Some(row) => {
                let key: String = row.get("key");
                key
            },
        };
        client.execute(
            "UPDATE keys SET last_used = CURRENT_TIMESTAMP
                WHERE key = $1",
            &[&key]
        ).await?;
        
        Ok(AuthResult::Ok)
    }
    
    /// Delete any keys that have been unused for longer than `self.key_life`.
    async fn cull_old_keys(&self) -> Result<(), DbError> {
        log::trace!("Db::cull_old_keys() called.");
        
        let client = self.connect().await?;
        let n_culled = client.execute(
            "DELETE FROM keys
                WHERE last_used + ($1 || ' ')::INTERVAL < now()",
            &[&self.key_life]
        ).await?;
        log::trace!("Deleted {} keys.", &n_culled);
        
        Ok(())
    }
    
    /** 
    Drop both database tables.
    
    This is largely for cleanup after testing.
    */
    async fn nuke_database(&self) -> Result<(), DbError> {
        log::trace!("Db::nuke_database() called");
        let mut client = self.connect().await?;
        let t = client.transaction().await
            .map_err(|e| format!("Auth DB Unable to begin transaction: {}", &e))?;
        
        let mut n_rows: u64 = 0;
        n_rows += t.execute("DROP TABLE keys", &[]).await
            .map_err(|e| format!("Error dropping keys table: {}", &e))?;
        n_rows += t.execute("DROP TABLE users", &[]).await
            .map_err(|e| format!("Error dropping users table: {}", &e))?;
        
        t.commit().await
            .map_err(|e| format!("Error committing nuclear transaction: {}", &e))?;
        
        log::trace!("Nuked {} rows.", &n_rows);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::ensure_logging;
    
    use serial_test::serial;
    
    static USERS: &[&str] = &["dan", "griffin", "krista"];
    static PASSWORDS: &[&str] = &["booga", "purple", "aqua"];
    static SALTS: &[&str] = &["asdf", "hjkl", "qwer"];
    
    static TEST_CONNECTION: &str = "host=localhost user=camp_test password='camp_test' dbname=camp_auth_test";
    
    #[tokio::test]
    #[ignore]
    #[serial]
    async fn reset_db() {
        ensure_logging();
        let db = Db::new(TEST_CONNECTION.to_owned());
        db.nuke_database().await.unwrap();
    }
    
    #[tokio::test]
    #[serial]
    async fn populate_db() {
        ensure_logging();
        
        let db = Db::new(TEST_CONNECTION.to_owned());
        db.ensure_db_schema().await.unwrap();
        
        let n_users: usize = USERS.len();
        assert_eq!(
            db.add_users(USERS, PASSWORDS, SALTS).await.unwrap(),
            n_users as u64
        );

        for n in 0..n_users {
            let (uname, pwd, salt) = (USERS[n], PASSWORDS[n], SALTS[n]);
            assert_eq!(
                db.check_password(uname, pwd, salt).await.unwrap(),
                AuthResult::Ok
            );
        }
        
        assert_eq!(
            db.check_password(USERS[1], "mama moo moo", SALTS[1]).await.unwrap(),
            AuthResult::BadPassword
        );
        assert_eq!(
            db.check_password(USERS[1], PASSWORDS[1], "not a real salt").await.unwrap(),
            AuthResult::BadPassword
        );
        
        db.delete_users(&USERS[1..]).await.unwrap();
        
        assert_eq!(
            db.check_password(USERS[1], PASSWORDS[1], SALTS[1]).await.unwrap(),
            AuthResult::NoSuchUser
        );
        
        db.nuke_database().await.unwrap();
        
        match db.check_password(USERS[1], PASSWORDS[1], SALTS[1]).await {
            Err(_) => { /* this is okay */ },
            x @ _ => { panic!("Expected Err(_), got {:?}", &x); },
        }
    }
    
    #[tokio::test]
    #[serial]
    async fn issue_keys() {
        use tokio::time::sleep;
        use std::time::Duration;
        
        ensure_logging();
        
        let mut db = Db::new(TEST_CONNECTION.to_owned());
        db.ensure_db_schema().await.unwrap();
        
        db.add_users(USERS, PASSWORDS, SALTS).await.unwrap();
        let key = match db.check_password_and_issue_key(
            USERS[0], PASSWORDS[0], SALTS[0]
        ).await.unwrap() {
            AuthResult::Key(k) => k,
            x @ _ => { panic!("Expected AuthResult::Key(_), got {:?}", &x); },
        };
        
        assert_eq!(db.check_key(USERS[0], &key).await.unwrap(), AuthResult::Ok);
        assert_eq!(db.check_key(USERS[1], &key).await.unwrap(), AuthResult::InvalidKey);
        assert_eq!(db.check_key(USERS[0], "wrong_key").await.unwrap(), AuthResult::InvalidKey);
        
        db.set_key_life(1_u64);
        let key = match db.check_password_and_issue_key(
            USERS[1], PASSWORDS[1], SALTS[1]
        ).await.unwrap() {
            AuthResult::Key(k) => k,
            x @ _ => { panic!("Expected AuthResult::Key(_), got {:?}", &x); },
        };
        assert_eq!(db.check_key(USERS[1], &key).await.unwrap(), AuthResult::Ok);
        sleep(Duration::from_millis(1500)).await;
        assert_eq!(db.check_key(USERS[1], &key).await.unwrap(), AuthResult::InvalidKey);
        db.cull_old_keys().await.unwrap();
        
        db.nuke_database().await.unwrap();
    }
}