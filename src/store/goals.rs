/*!
`Store` methods et. al. for dealing with `Goal` insertion, update,
and retrieval.

```sql
CREATE TABLE goals (
    id          BIGSERIAL PRIMARY KEY,
    uname       TEXT REFERENCES students(uname),
    sym         TEXT REFERENCES courses(sym),
    seq         SMALLINT,
    custom      BIGINT REFERENCES custom_chapters(id),
    review      BOOL,
    incomplete  BOOL,
    due         DATE,
    done        DATE,
    tries       SMALLINT,
    score   TEXT
);
```
*/
use futures::stream::{FuturesUnordered, StreamExt};
use tokio_postgres::{Row, types::ToSql, types::Type};

use super::{Store, DbError};
use crate::{
    course::{Course, Chapter},
    pace::{BookCh, CustomCh, Goal, Pace, Source},
    user::{Student, Teacher},
};

fn goal_from_row(row: &Row) -> Result<Goal, DbError> {
    let bkch = BookCh {
        sym: row.try_get("sym")?,
        seq: row.try_get("seq")?,
        // Gets set in the `Pace` constructor.
        level: 0.0,
    };

    Ok(Goal {
        id: row.try_get("id")?,
        uname: row.try_get("uname")?,
        source: Source::Book(bkch),
        review: row.try_get("review")?,
        incomplete: row.try_get("incomplete")?,
        due: row.try_get("due")?,
        done: row.try_get("done")?,
        tries: row.try_get("tries")?,
        // Gets set in the `Pace` constructor.
        weight: 0.0,
        score: row.try_get("score")?
    })
}

impl Store {
    pub async fn insert_goals(
        &self,
        goals: &[Goal]
    ) -> Result<usize, DbError> {
        log::trace!("Store::insert_goals( [ {} goals ] ) called.", &goals.len());

        // Make copies of all the book `Source`s, and throw an error on custom
        // ones because we don't support those yet.
        for g in goals.iter() {
            if let Source::Custom(_) = &g.source {
                return Err(DbError("Custom Sources are unsupported.".to_owned()));
            }
        }
        let sources: Vec<BookCh> = goals.iter()
            .map(|g| match g.source {
                Source::Book(ref bch) => bch.clone(),
                _ => panic!("We just checked, and there shouldn't be any Custom Sources."),
            }).collect();

        let mut client = self.connect().await?;
        let t = client.transaction().await?;

        let insert_stmt = t.prepare_typed(
            "INSERT INTO goals (
                uname, sym, seq, review, incomplete,
                due, done
            )
            VALUES (
                $1, $2, $3, $4, $5,
                $6, $7
            )",
            &[
                Type::TEXT, Type::TEXT, Type::INT2, Type::BOOL, Type::BOOL,
                Type::DATE, Type::DATE
            ]
        ).await?;

        let pvec: Vec<[&(dyn ToSql + Sync); 7]> = goals.iter().zip(sources.iter())
            .map(|(g, src)| {
                let p: [&(dyn ToSql + Sync); 7] = [
                    &g.uname, &src.sym, &src.seq, &g.review, &g.incomplete,
                    &g.due, &g.done
                ];
                p
            }).collect();

        let mut n_inserted: u64 = 0;
        {
            let mut inserts = FuturesUnordered::new();
            for params in pvec.iter() {
                inserts.push(
                    t.execute(&insert_stmt, params)
                );
            }

            while let Some(res) = inserts.next().await {
                match res {
                    Ok(_) => { n_inserted += 1; },
                    Err(e) => {
                        let estr = format!(
                            "Error inserting Goal into database: {}", &e
                        );
                        return Err(DbError(estr));
                    },
                }
            }
        }

        t.commit().await?;

        Ok(n_inserted as usize)
    }

    pub async fn get_goals_by_student(
        &self,
        uname: &str
    ) -> Result<Vec<Goal>, DbError> {
        log::trace!("Store::get_goals_by_student( {:?} ) called.", uname);

        let client = self.connect().await?;
        
        let rows = client.query(
            "SELECT * FROM goals WHERE uname = $1",
            &[&uname]
        ).await?;

        let mut goals: Vec<Goal> = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            match goal_from_row(&row) {
                Ok(g) => { goals.push(g); },
                Err(e) => {
                    return Err(DbError(format!(
                        "Unable to read Goal from database: {}", &e
                    )));
                }
            }
        }

        Ok(goals)
    }
}