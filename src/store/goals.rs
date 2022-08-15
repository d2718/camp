/*!
`Store` methods et. al. for dealing with `Goal` insertion, update,
and retrieval.

```sql
CREATE TABLE goals (
    id          BIGSERIAL PRIMARY KEY,
    uname       TEXT REFERENCES students(uname),
    sym         TEXT REFERENCES courses(sym),
    chapt_id    BIGINT REFERENCES chapters(id),
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
use tokio_postgres::{Row, types::Type};

use super::{Store, DbError};
use crate::{
    course::{Course, Chapter},
    pace::{BookCh, CustomCh, Goal, Pace, Source},
    user::{Student, Teacher},
};

impl Store {
    pub async fn get_student_goals(&self, uname: &str) -> Result<Vec<Goal>, DbError> {
        log::trace!("Store::get_student_goals( {:?} ) called.", uname);


    }
}