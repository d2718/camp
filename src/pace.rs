/*!
The `Goal` struct and Pace calendars.
*/
use time::{Date, Month};

pub struct Goal {
    pub id: i64,
    pub uname: String,
    pub sym: Option<String>,
    pub seq: Option<i16>,
    pub custom: Option<i64>,
    pub review: bool,
    pub incomplete: bool,
    pub due: Option<Date>,
    pub done: Option<Date>,
    pub tries: i16,
    pub score: Option<String>,
}

fn blank_means_none(s: Option<&str>) -> Option<&str> {
    match s {
        Some(s) => match s.trim() {
            "" => None,
            x => Some(x),
        },
        None => None,
    }
}

impl Goal {
    /**
    Goal .csv rows should look like this

    ```csv
    #uname, sym, seq,     y, m,  d, rev, inc
    jsmith, pha1,  3, 2022, 09, 10,   x,
          ,     ,  9,     ,   , 28,    ,  x
    ```

    Columns `uname`, `sym`, `y`, `m` all default to the value of the previous
    goal, so to save work, you don't need to include them if they're the same
    as the previous line.

    Columns `rev` and `inc` are considered `true` if they have any text
    whatsoever.
     */
    pub fn from_csv_line(
        row: &csv::StringRecord,
        prev: Option<&Goal>
    ) -> Result<Goal, String> {
        log::trace!("Goal::from_csv_line( {:?} ) called.", row);

        let uname = match blank_means_none(row.get(0)) {
            Some(s) => s.to_owned(),
            None => match prev {
                Some(g) => g.uname.clone(),
                None => { return Err("No uname".into()); },
            },
        };

        let sym = match blank_means_none(row.get(1)) {
            Some(s) => s.to_owned(),
            None => match prev {
                Some(g) => match &g.sym {
                    Some(sym) => sym.clone(),
                    None => { return Err("No course symbol.".into()); },
                },
                None => { return Err("No course symbol".into()); },
            },
        };

        let seq: i16 = match blank_means_none(row.get(2)) {
            Some(s) => match s.parse() {
                Ok(n) => n,
                Err(_) => { return Err(format!("Unable to parse {:?} as number.", s)); },
            },
            None => { return Err("No chapter number.".into()); },
        };

        let y: i32 = match blank_means_none(row.get(3)) {
            Some(s) => match s.parse() {
                Ok(n) => n,
                Err(_) => { return Err(format!("Unable to parse {:?} as year.", s)); },
            },
            None => match prev {
                Some(g) => match g.due {
                    Some(d) => d.year(),
                    None => { return Err("No year".into()); }
                },
                None => { return Err("No year".into()); },
            }
        };

        let m: Month = match blank_means_none(row.get(4)) {
            Some(s) => match s.parse::<u8>() {
                Ok(n) => match Month::try_from(n-1) {
                    Ok(m) => m,
                    Err(_) => { return Err(format!("Not an appropriate Month value: {}", n)); },
                },
                Err(_) => { return Err(format!("Unable to parse {:?} as month number.", s)); },
            },
            None => match prev {
                Some(g) => match g.due {
                    Some(d) => d.month(),
                    None => { return Err("No month".into()); },
                },
                None => { return Err("No month".into()); },
            },
        };

        let d: u8 = match blank_means_none(row.get(5)) {
            Some(s) => match s.parse() {
                Ok(n) => n,
                Err(_) => { return Err(format!("Unable to parse {:?} as day number.", s)); },
            },
            None => { return Err("No day".into()); },
        };

        let due = match Date::from_calendar_date(y, m, d) {
            Ok(d) => d,
            Err(_) => { return Err(format!("{}-{}-{} is not a valid date", &y, &m, &d)); },
        };

        let review = match blank_means_none(row.get(6)) {
            Some(_) => true,
            None => false,
        };

        let incomplete = match blank_means_none(row.get(7)) {
            Some(_) => true,
            None => false,
        };

        let g = Goal {
            // This doesn't matter; it will be set upon database insertion.
            id: 0,
            uname,
            sym: Some(sym),
            seq: Some(seq),
            // No goals read from .csv files will be custom chatpers.
            custom: None,
            review,
            incomplete,
            due: Some(due),
            // No goals read from .csv files can possibly be done.
            done: None,
            // Will get set once it's done.
            tries: 0,
            // Goals read from .csv files should have no score yet.
            score: None,
        };

        Ok(g)
    }
}