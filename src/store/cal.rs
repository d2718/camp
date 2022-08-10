/*!
Calendar oriented methods.

Dates are represented by the `time::Date` struct.

```sql
CREATE TABLE calendar (
    day DATE UNIQUE NOT NULL
);
```
*/
use futures::stream::{FuturesUnordered, StreamExt};
use tokio_postgres::types::{ToSql, Type};
use time::Date;

use super::{Store, DbError};

impl Store {
    pub async fn insert_dates(
        &self,
        dates: &[Date]
    ) -> Result<usize, DbError> {
        log::trace!("Store::insert_dates( {:?} ) called.", &dates);

        let mut client = self.connect().await?;
        let t = client.transaction().await?;

        let insert_statement = t.prepare_typed(
            "INSERT INTO calendar (day) VALUES ($1)
                ON CONFLICT DO UPDATE",
            &[Type::DATE]
        ).await?;

        let n_dates = dates.len();
        match n_dates {
            0 => { return Ok(0); },
            1 => {
                let n_rows = t.execute(
                    &insert_statement,
                    &[&dates[0]]
                ).await?;

                t.commit().await?;
                return Ok(n_rows as usize);
            },
            _ => { /* We go on to the complicated case. */ }
        }

        let mut n_inserted: u64 = 0;
        {
            let date_refs: Vec<[&(dyn ToSql + Sync); 1]> = dates.iter()
                .map(|d| {
                    let p: [&(dyn ToSql + Sync); 1] = [d];
                    p
                }).collect();
            
            let mut inserts = FuturesUnordered::new();
            for params in date_refs.iter() {
                inserts.push(
                    t.execute(
                        &insert_statement,
                        &params[..]
                    )
                );
            }

            while let Some(res) = inserts.next().await {
                match res {
                    Ok(_) => { n_inserted += 1; },
                    Err(e) => {
                        let estr = format!("Error inserting date into calendar: {}", &e);
                        return Err(DbError(estr));
                    },
                }
            };
        }

        t.commit().await?;
        Ok(n_inserted as usize)
    }

    pub async fn delete_dates(
        &self,
        dates: &[Date]
    ) -> Result<usize, DbError> {
        log::trace!("Store::delete_dates( {:?} ) called.", &dates);

        let mut client = self.connect().await?;
        let t = client.transaction().await?;

        let delete_statement = t.prepare_typed(
            "DELETE FROM calendar WHERE date = ANY($1)",
            &[Type::DATE_ARRAY]
        ).await?;

        let n_removed = match t.execute(
            &delete_statement,
            &[&dates]
        ).await {
            Ok(n) => n,
            Err(e) => {
                return Err(DbError(format!(
                    "Error removing dates from the calendar: {}", &e
                )));
            },
        };

        t.commit().await?;
        Ok(n_removed as usize)
    }
}