/*!
Database interaction module.

The Postgres database to which this connects is meant to have the following
sets of tables.

This first set is to store information about the courses.

```sql

CREATE TABLE courses (
    id    SERIAL PRIMARY KEY,
    sym   TEXT UNIQUE NOT NULL,
    book  TEXT,
    title TEXT NOT NULL,
    level REAL
);

CREATE TABLE chapters (
    id       SERIAL PRIMARY KEY,
    course   INTEGER REFERENCES courses(id),
    sequence SMALLINT,
    title    TEXT,      /* NULL should give default-generated title */
    subject  TEXT,      /* NULL should just be a blank */
    weight   REAL       /* NULL should give default value of 1.0 */
);

CREATE TABLE custom_chapters (
    id    BIGSERIAL PRIMARY KEY,
    uname REFERENCES user(uname),   /* username of creator */
    title TEXT NOT NULL,
    weight REAL     /* NULL should give default value of 1.0 */
);
```

TODO:
  * Better `.map_err()` annotations.

*/
use std::collections::HashMap;
use std::fmt::Write;

use tokio_postgres::{Client, NoTls, Row, Statement, types::Type};
use rand::{Rng, distributions};

use crate::course::{Course, Chapter, Custom};

const DEFAULT_SALT_LENGTH: usize = 4;
const DEFAULT_SALT_CHARS: &str =
"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

static SCHEMA: &[(&str, &str, &str)] = &[
    // Three tables of course info: courses, chapters, and custom "chapters".

    (
        "SELECT FROM information_schema.tables WHERE table_name = 'courses'",
        "CREATE TABLE courses (
            id    BIGSERIAL PRIMARY KEY,
            sym   TEXT UNIQUE NOT NULL,
            title TEXT NOT NULL,
            book  TEXT,
            level REAL
        )",
        "DROP TABLE courses",
    ),

    (
        "SELECT FROM information_schema.tables WHERE table_name = 'chapters'",
        "CREATE TABLE chapters (
            id          BIGSERIAL PRIMARY KEY,
            course      BIGINT REFERENCES courses(id),
            sequence    SMALLINT,
            title       TEXT,   /* default is generated 'Chapter N' title */
            subject     TEXT,   /* default is blank */
            weight      REAL    /* default is 1.0 */
        )",
        "DROP TABLE chapters",
    ),

    (
        "SELECT FROM information_schema.tables WHERE table_name = 'custom_chapters'",
        "CREATE TABLE custom_chapters (
            id      BIGSERIAL PRIMARY KEY,
            uname   TEXT,   /* REFERENCES user(uname), when 'users' table available */
            title   TEXT NOT NULL,
            weight  REAL    /* default should be 1.0 */
        )",
        "DROP TABLE custom_chapters",
    ),
];

#[derive(Debug, PartialEq)]
pub struct DbError(String);

impl DbError {
    /// Prepend some contextual `annotation` for the error.
    fn annotate(self, annotation: &str) -> Self {
        let s = format!("{}: {}", annotation, &self.0);
        Self(s)
    }

    pub fn display(&self) -> &str { &self.0 }
}

impl From<tokio_postgres::error::Error> for DbError {
    fn from(e: tokio_postgres::error::Error) -> DbError {
        let mut s = format!("Data DB: {}", &e);
        if let Some(dbe) = e.as_db_error() {
            write!(&mut s, "; {}", dbe).unwrap();
        }
        DbError(s)
    }
}

impl From<String> for DbError {
    fn from(s: String) -> DbError { DbError(s) }
}

pub struct Store {
    connection_string: String,
    salt_chars: Vec<char>,
    salt_length: usize,
}

impl Store {
    pub fn new(connection_string: String) -> Self {
        log::trace!("Store::new( {:?} ) called.", &connection_string);

        let salt_chars: Vec<char> = DEFAULT_SALT_CHARS.chars().collect();
        let salt_length = DEFAULT_SALT_LENGTH;

        Self { connection_string, salt_chars, salt_length }
    }

    /// Set characters to use when generating user salt strings.
    ///
    /// Will quietly do nothing if `new_chars` has zero length.
    pub fn set_salt_chars(&mut self, new_chars: &str) {
        if new_chars.len() > 0 {
            self.salt_chars = new_chars.chars().collect();
        }
    }

    /// Set the length of salt strings to generate.
    ///
    /// Will quietly do nothing if set to zero.
    pub fn set_salt_length(&mut self, new_length: usize) {
        if new_length > 0 {
            self.salt_length = new_length;
        }
    }

    /// Generate a new user salt based on the current values of
    /// self.salt_chars and self.salt_length.
    fn generate_salt(&self) -> String {
        // self.salt_chars should never have zero length.
        let dist = distributions::Slice::new(&self.salt_chars).unwrap();
        let rng = rand::thread_rng();
        let new_salt: String = rng.sample_iter(&dist)
            .take(self.salt_length)
            .collect();
        new_salt
    }

    async fn connect(&self) -> Result<Client, DbError> {
        log::trace!(
            "Store::connect() called w/connection string {:?}",
            &self.connection_string
        );

        match tokio_postgres::connect(&self.connection_string, NoTls).await {
            Ok((client, connection)) => {
                log::trace!("    ...connection successful.");
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        log::error!("Data DB connection error: {}", &e);
                    } else {
                        log::trace!("tokio connection runtime drops.");
                    }
                });
                Ok(client)
            },
            Err(e) => {
                let dberr = DbError::from(e);
                log::trace!("    ...connection failed: {:?}", &dberr);
                Err(dberr.annotate("Unable to connect"))
            }
        }
    }

    pub async fn ensure_db_schema(&self) -> Result<(), DbError> {
        log::trace!("Store::ensure_db_schema() called.");

        let mut client = self.connect().await?;
        let t = client.transaction().await
            .map_err(|e| DbError::from(e)
                .annotate("Data DB unable to begin transaction"))?;
            
        for (test_stmt, create_stmt, _) in SCHEMA.iter() {
            if t.query_opt(test_stmt.to_owned(), &[]).await?.is_none() {
                log::info!(
                    "{:?} returned no results; attempting to insert table.",
                    test_stmt
                );
                t.execute(create_stmt.to_owned(), &[]).await?;
            }
        }

        t.commit().await
            .map_err(|e| DbError::from(e)
                .annotate("Error committing transaction"))
    }

    /**
    Drop all database tables to fully reset database state.

    This is only meant for cleanup after testing. It is advisable to look at
    the ERROR level log output when testing to ensure this method did its job.
    */
    #[cfg(test)]
    pub async fn nuke_database(&self) -> Result<(), DbError> {
        log::trace!("Store::nuke_database() called.");

        let client = self.connect().await?;

        for (_, _, drop_stmt) in SCHEMA.iter().rev() {
            if let Err(e) = client.execute(drop_stmt.to_owned(), &[]).await {
                let err = DbError::from(e);
                log::error!("Error dropping: {:?}: {}", &drop_stmt, &err.display());
            }
        }

        log::trace!("    ....nuking comlete.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    /*!
    These tests assume you have a Postgres instance running on your local
    machine with resources named according to what you see in the
    `static TEST_CONNECTION &str`:

    ```text
    user: camp_test
    password: camp_test

    with write access to:

    database: camp_store_test
    ```
    */
    use super::*;
    use crate::tests::ensure_logging;

    use std::fs;

    use float_cmp::approx_eq;
    use serial_test::serial;

    static TEST_CONNECTION: &str = "host=localhost user=camp_test password='camp_test' dbname=camp_store_test";

    /**
    This function is for getting the database back in a blank slate state if
    a test panics partway through and leaves it munged.

    ```bash
    cargo test reset_store -- --ignored
    ```
    */
    #[tokio::test]
    #[ignore]
    #[serial]
    async fn reset_store() {
        ensure_logging();
        let db = Store::new(TEST_CONNECTION.to_owned());
        db.nuke_database().await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn create_store() {
        ensure_logging();

        let db = Store::new(TEST_CONNECTION.to_owned());
        db.ensure_db_schema().await.unwrap();
        db.nuke_database().await.unwrap();
    }
}