/*
`Store` methods et. al. for dealing with the different kinds of users.

```sql
CREATE TABLE users (
    uname TEXT PRIMARY KEY,
    role  TEXT,      /* one of { 'admin', 'boss', 'teacher', 'student' } */
    salt  TEXT,
    email TEXT
);

CREATE TABLE teachers (
    uname TEXT REFERENCES users(uname),
    name  TEXT
);

CREATE TABLE students (
    uname   TEXT REFERENCES users(uname),
    last    TEXT,
    rest    TEXT,
    teacher TEXT REFERENCES teachers(uname),
    parent  TEXT     /* parent email address */
);

```
*/
use std::collections::HashMap;
use std::fmt::Write;

use futures::stream::{FuturesUnordered, StreamExt};
use tokio_postgres::{Row, Transaction, types::{ToSql, Type}};

use super::{Store, DbError};
use crate::user::*;

fn base_user_from_row(row: &Row) -> Result<BaseUser, DbError> {
    log::trace!("base_user_from_row( {:?} ) called", row);

    let role_str: &str = row.try_get("role")?;
    let bu = BaseUser {
        uname: row.try_get("uname")?,
        role: role_str.parse()?,
        salt: row.try_get("salt")?,
        email: row.try_get("email")?,
    };

    log::trace!("    ...base_user_from_row() returning {:?}", &bu);
    Ok(bu)
}

/// Return the role of extant user `uname`, if he exists.
///
/// This function is used when inserting new users to ensure, mainly to
/// ensure good error messaging when a username is already in use.
async fn check_existing_user_role(
    t: &Transaction<'_>,
    uname: &str,
) -> Result<Option<Role>, DbError> {
    log::trace!("check_existing_user_role( T, {:?} ) called.", uname);

    match t.query_opt(
        "SELECT role FROM users WHERE uname = $1",
        &[&uname]
    ).await.map_err(|e|
        DbError(format!("{}", &e))
            .annotate("Error querying for preexisting uname")
    )? {
        None => Ok(None),
        Some(row) => {
            let role_str: &str = row.try_get("role")
                .map_err(|e|
                    DbError(format!("{}", &e))
                        .annotate("Error getting role of preexisting uname")
                )?;
            let role: Role = role_str.parse()
                .map_err(|e|
                    DbError(format!("{}", &e))
                        .annotate("Error parsing role of preexisting uname")
                )?;
            Ok(Some(role))
        },
    }
}

impl Store {
    
    /**
    Deletes a user from the database, regardless of role.

    It's not clever; it tries to shotgun delete both student and teacher
    records for a given `uname` before deleting the entry from the `users`
    table. I haven't tested it, but I think this is probably faster than
    "the right thing": querying the role associated with the `uname` and
    performing an appropriate additional delete if necessary.
    */
    pub async fn delete_user(
        &self,
        uname: &str,
    ) -> Result<(), DbError> {
        log::trace!("Store::delete_user( {:?} ) called.", uname);

        let mut client = self.connect().await?;
        let t = client.transaction().await?;

        /*
        JFC the type annotations here.

        This obnoxious way of passing parameters to the two following SQL
        DELETE statements is necessary to satisfy the borrow checker. Sorry.
        I absolutely invite you to make this suck less if you can.
        */
        let params: [&(dyn ToSql + Sync); 1] = [&uname];

        let (s_del_res, t_del_res) = tokio::join!(
            t.execute(
                "DELETE FROM students WHERE uname = $1",
                &params[..]
            ),
            t.execute(
                "DELETE FROM teachers WHERE uname = $1",
                &params[..]
            ),
        );

        match s_del_res {
            Err(e) => { return Err(e.into()); },
            Ok(0) => {},
            Ok(1) => { log::trace!("{} student record deleted.", uname); },
            Ok(n) => { 
                log::warn!(
                    "Deleting single student {} record affected {} rows.",
                    uname, &n
                );
            },
         }
         match t_del_res {
            Err(e) => { return Err(e.into()); },
            Ok(0) => {},
            Ok(1) => { log::trace!("{} teacher record deleted.", uname); },
            Ok(n) => { 
                log::warn!(
                    "Deleting single teacher {} record affected {} rows.",
                    uname, &n
                );
            },
         }

        let n = t.execute(
            "DELETE FROM users WHERE uname = $1",
            &[&uname]
        ).await?;

        if n == 0 {
            Err(DbError(format!("There is no user with uname {:?}.", uname)))
        } else {
            t.commit().await?;
            Ok(())
        }
    }

    /// Inserts the `user::BaseUser` information into the `users` table in the
    /// database.
    ///
    /// This is used by the `Store::insert_xxx()` methods to insert this part
    /// of the information. It also calls `check_existing_user_role()` and
    /// throws a propagable error if the given uname already exists.
    async fn insert_base_user(
        &self,
        t: &Transaction<'_>,
        uname: &str,
        email: &str,
        role: Role,
    ) -> Result<(), DbError> {
        log::trace!(
            "insert_base_user( T, {:?}, {:?}, {} ) called.",
            uname, email, role
        );
    
        if let Some(role) = check_existing_user_role(t, uname).await? {
            return Err(DbError(format!(
                "User name {} already exists with role {}.",
                uname, &role
            )));
        }
    
        t.execute(
            "INSERT INTO users (uname, role, salt, email)
                VALUES ($1, $2, $3, $4)",
            &[
                &uname,
                &role.to_string(),
                &self.generate_salt(),
                &email,
            ]
        ).await?;
    
        Ok(())
    }

    pub async fn insert_admin(
        &self,
        uname: &str,
        email: &str,
    ) -> Result<(), DbError> {
        log::trace!("Store::insert_admin( {:?},{:?} ) called.", uname, email);

        let mut client = self.connect().await?;
        let t = client.transaction().await?;

        self.insert_base_user(&t, uname, email, Role::Admin).await?;

        t.commit().await?;
        log::trace!("Inserted Admin {:?} ({}).", uname, email);
        Ok(())
    }

    pub async fn insert_boss(
        &self,
        uname: &str,
        email: &str,
    ) -> Result<(), DbError> {
        log::trace!("Store::insert_boss( {:?}, {:?} ) called.", uname, email);

        let mut client = self.connect().await?;
        let t = client.transaction().await?;

        self.insert_base_user(&t, uname, email, Role::Boss).await?;

        t.commit().await?;
        log::trace!("Inserted Boss {:?} ({})", uname, email);
        Ok(())
    }

    pub async fn insert_teacher(
        &self,
        uname: &str,
        email: &str,
        name: &str,
    ) -> Result<(), DbError> {
        log::trace!(
            "Store::insert_teacher( {:?}, {:?}, {:?} ) called.",
            uname, email, name
        );

        let mut client = self.connect().await?;
        let t = client.transaction().await?;

        self.insert_base_user(&t, uname, email, Role::Teacher).await?;

        t.execute(
            "INSERT INTO teachers (uname, name)
                VALUES ($1, $2)",
            &[&uname, &name]
        ).await?;

        t.commit().await?;
        log::trace!("Inserted Teacher {:?}, ({}, {})", uname, name, email);
        Ok(())
    }

    pub async fn insert_students(
        &self,
        students: &[Student]
    ) -> Result<usize, DbError> {
        log::trace!("Store::insert_students( [ {} students ] ) called.", students.len());

        let new_unames: Vec<&str> = students.iter()
            .map(|s| s.base.uname.as_str())
            .collect();

        let mut client = self.connect().await?;
        let t = client.transaction().await?;
        let preexisting_uname_query = t.prepare_typed(
            "SELECT uname, role FROM users WHERE uname = ANY($1)",
            &[Type::TEXT_ARRAY]
        ).await?;

        // Check to see if any of the new students have unames already in use
        // and return an informative error if so.
        let preexisting_uname_rows = t.query(
            &preexisting_uname_query,
            &[&new_unames]
        ).await?;
        if preexisting_uname_rows.len() > 0 {
            /* Find the length of the longest uname; it will be used to format
            our error message. This finds the maximum length _in bytes_ (and
            not characters), but this is almost undoubtedly fine here.
            
            Also, unwrapping is okay, because there's guaranteed to be at
            least one item in the iterator, and usizes have total order. */
            let uname_len = new_unames.iter().map(|uname| uname.len()).max().unwrap();
            let mut estr = String::from("Database already contains users with the following unames:\n");
            for row in preexisting_uname_rows.iter() {
                let uname: &str = row.try_get("uname")?;
                let role: &str = row.try_get("role")?;
                write!(
                    &mut estr,
                    "{:width$} ({})",
                    uname, role, width = uname_len
                ).map_err(|e| format!(
                    "There was an error preparing an error message: {}", &e
                ))?;
            }
            return Err(DbError(estr));
        }

        let (buiq, stiq) = tokio::join!(
            t.prepare_typed(
                "INSERT INTO users (uname, role, salt, email)
                    VALUES ($1, $2, $3, $4)",
                &[Type::TEXT, Type::TEXT, Type::TEXT, Type::TEXT]
            ),
            t.prepare_typed(
                "INSERT INTO students (uname, last, rest, teacher, parent)
                    VALUES ($1, $2, $3, $4, $5)",
                &[Type::TEXT, Type::TEXT, Type::TEXT, Type::TEXT, Type::TEXT]
            ),
        );
        let (base_user_insert_query, student_table_insert_query) = (buiq?, stiq?);

        /*
        This next block is terrible and confusing.

        I want to run a bunch of database inserts concurrently. The
        parameters referenced in the insert statements, though, must
        be in a slice of references. These slices need to be bound
        _oustide_ the async function call that's being passed into
        `FuturesUnordered`.

        The `Student`s all exist in a slice that's been passed to
        this function, so we can refer to those unames and emails.

        We create a `String` holding the role (`"Student"`) each of
        these students will be assigned.

        We create a vector of salt strings we can reference.

        Finally we create a vector of four-element arrays (`pvec`).
        Each array holds references to the four parameters we are
        passing to the insert function to insert the corresponding
        student:
          * a reference to the `Student.base.uname`
          * a reference to the String holding the text "role"
          * a reference to one of the salts
          * a reference to the `Student.base.email`
        
        A reference to this array (making it a slice), will then be
        passed as the "parameters" to the insert statement.

        Phew.

        We are also putting it in its own scope, so `inserts` will drop.
        */
        let mut n_base_inserted: u64 = 0;
        {
            let student_role = Role::Student.to_string();
            let salts: Vec<String> = std::iter::repeat(())
                .take(students.len())
                .map(|_| self.generate_salt())
                .collect();
            let pvec: Vec<[&(dyn ToSql + Sync); 4]> = students.iter()
                .enumerate()
                .map(|(n, s)| {
                    let p: [&(dyn ToSql + Sync); 4] =
                        [&s.base.uname, &student_role, &salts[n], &s.base.email];
                    p
                }).collect();

            let mut inserts = FuturesUnordered::new();
            for params in pvec.iter() {
                inserts.push(
                    t.execute(
                        &base_user_insert_query,
                        params
                    )
                );
            }

            while let Some(res) = inserts.next().await {
                match res {
                    Ok(_) => { n_base_inserted += 1; },
                    Err(e) => {
                        let estr = format!(
                            "Error inserting base user into database: {}", &e
                        );
                        return Err(DbError(estr));
                    }
                }
            }
        }

        /*
        We're about to do a similar thing here. See the previous massive
        comment block if you're confused.
        */
        let mut n_stud_inserted: u64 = 0;
        {
            let pvec: Vec<[&(dyn ToSql + Sync); 5]> = students.iter()
                .map(|s| {
                    let p: [&(dyn ToSql + Sync); 5] =
                        [&s.base.uname, &s.last, &s.rest, &s.teacher, &s.parent];
                    p
                }).collect();
            
            let mut inserts = FuturesUnordered::new();
            for params in pvec.iter() {
                inserts.push(
                    t.execute(
                        &student_table_insert_query,
                        params
                    )
                );
            }

            while let Some(res) = inserts.next().await {
                match res {
                    Ok(_) => { n_stud_inserted += 1; },
                    Err(e) => {
                        let estr = format!(
                            "Error inserting into students table in database: {}", &e
                        );
                        return Err(DbError(estr));
                    }
                }
            }
        }

        t.commit().await?;

        log::trace!(
            "Inserted {} base users and {} student table rows.",
            &n_base_inserted, &n_stud_inserted
        );
        Ok(n_stud_inserted as usize)
    }

    pub async fn get_users(&self) -> Result<HashMap<String, BaseUser>, DbError> {
        log::trace!("Store::get_users() called.");

        let client = self.connect().await?;
        let rows = client.query("SELECT * FROM users;", &[]).await?;
        let mut map: HashMap<String, BaseUser> = HashMap::with_capacity(rows.len());

        for row in rows.iter() {
            let u = base_user_from_row(row)?;
            map.insert(u.uname.clone(), u);
        }

        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serial_test::serial;

    use crate::tests::ensure_logging;
    use crate::store::tests::TEST_CONNECTION;

    static ADMINS: &[(&str, &str)] = &[
        ("admin", "thelma@camelotacademy.org"),
        ("dan", "dan@camelotacademy.org"),
    ];

    static BOSSES: &[(&str, &str)] = &[
        ("boss", "boss@camelotacademy.org"),
        ("tdg", "thelma@camelotacademy.org"),
    ];

    static TEACHERS: &[(&str, &str, &str)] = &[
        ("berro", "berro@camelotacademy.org", "Mr Berro"),
        ("jenny", "jenny@camelotacademy.org", "Ms Jenny"),
        ("irfan", "irfan@camelotacademy.org", "Mr Irfan"),
    ];

    #[tokio::test]
    #[serial]
    async fn insert_users() {
        ensure_logging();

        let db = Store::new(TEST_CONNECTION.to_owned());
        db.ensure_db_schema().await.unwrap();

        for (uname, email) in ADMINS.iter() {
            db.insert_admin(uname, email).await.unwrap();
        }
        for (uname, email) in BOSSES.iter() {
            db.insert_boss(uname, email).await.unwrap();
        }
        for (uname, email, name) in TEACHERS.iter() {
            db.insert_teacher(uname, email, name).await.unwrap();
        }

        let mut umap = db.get_users().await.unwrap();

        for (uname, email) in ADMINS.iter() {
            let u = umap.remove(*uname).unwrap();
            assert_eq!(
                (*uname, *email, Role::Admin),
                (u.uname.as_str(), u.email.as_str(), u.role)
            );
            db.delete_user(uname).await.unwrap();
        }
        for (uname, email) in BOSSES.iter() {
            let u = umap.remove(*uname).unwrap();
            assert_eq!(
                (*uname, *email, Role::Boss),
                (u.uname.as_str(), u.email.as_str(), u.role)
            );
            db.delete_user(uname).await.unwrap();
        }

        for (uname, email, _) in TEACHERS.iter() {
            let u = umap.remove(*uname).unwrap();
            assert_eq!(
                (*uname, *email, Role::Teacher),
                (u.uname.as_str(), u.email.as_str(), u.role)
            );
            db.delete_user(uname).await.unwrap();
        }

        db.nuke_database().await.unwrap();
    }
}