/*!
Calendar oriented methods.

Dates are represented by the `time::Date` struct.

```sql
CREATE TABLE calendar (
    day DATE UNIQUE NOT NULL
);
```

```sql
CREATE TABLE dates (
    name TEXT PRIMARY KEY,
    day DATE NOT NULL
);
```
*/
use futures::stream::{FuturesUnordered, StreamExt};
use tokio_postgres::types::{ToSql, Type};
use time::Date;

use super::{Store, DbError};

impl Store {
    pub async fn set_calendar(
        &self,
        dates: &[Date]
    ) -> Result<(usize, usize), DbError> {
        log::trace!("Store::insert_dates( {:?} ) called.", &dates);

        let mut client = self.connect().await?;
        let t = client.transaction().await?;

        let insert_statement = t.prepare_typed(
            "INSERT INTO calendar (day) VALUES ($1)
                ON CONFLICT DO UPDATE",
            &[Type::DATE]
        ).await?;

        let n_deleted = t.execute("DELETE FROM calendar", &[]).await
            .map_err(|e| format!("Unable to clear old calendar: {}", &e))?;

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
        Ok((n_deleted as usize, n_inserted as usize))
    }

    pub async fn get_calendar(&self) -> Result<Vec<Date>, DbError> {
        log::trace!("Store::get_dates() called.");

        let client = self.connect().await?;
        let rows = client.query(
            "SELECT day FROM calendar ORDER BY day", &[]
        ).await.map_err(|e| format!(
            "Error fetching calendar from Data DB: {}", &e
        ))?;

        let mut dates: Vec<Date> = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let d: Date = row.try_get("day")?;
            dates.push(d);
        }

        Ok(dates)
    }

    pub async fn add_one_day(&self, day: Date) -> Result<(), DbError> {
        log::trace!("Store::set_one_day( {} ) called.", &day);

        let client = self.connect().await?;
        client.execute(
            "INSERT INTO calendar (day) VALUES ($1)
                ON CONFLICT DO UPDATE",
            &[&day]
        ).await.map_err(|e| format!("Unable to add {} to calendar: {}", &day, &e))?;

        Ok(())
    }

    pub async fn delete_one_day(&self, day: Date) -> Result<(), DbError> {
        log::trace!("Store::clear_one_day( {} ) called.", &day);

        let client = self.connect().await?;
        client.execute(
            "DELETE FROM calendar WHERE day = $1",
            &[&day]
        ).await.map_err(|e| format!("Unable to delete {} from calendar: {}", &day, &e))?;

        Ok(())
    }

    pub async fn set_date(
        &self,
        date_name: &str,
        day: Date
    ) -> Result<(), DbError> {
        log::trace!("Store::set_date( {:?}, {} ) called.", date_name, &day);

        let client = self.connect().await?;
        client.execute(
            "INSERT INTO dates (name, day) VALUES ($1, $2)
                ON CONFLICT TO UPDATE",
            &[&date_name, &day]
        ).await.map_err(|e|
            format!("Unable to set {:?} date {}: {}", date_name, &day, &e))?;
        
        Ok(())
    }
}