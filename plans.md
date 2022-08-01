
## Auth

Separate database from the rest of the sections.

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

## Users

```sql

CREATE TABLE users (
    uname TEXT PRIMARY KEY,
    role  TEXT,      /* one of { 'admin', 'boss', 'teacher', 'student' } */
    salt  TEXT,
    email TEXT
);

/* users.role should properly be an ENUM, but that type doesn't seem to
 * be well-supported by the tokio-postgres crate.
 */
 
CREATE TABLE teachers (
    uname TEXT REFERENCES users(uname),
    name  TEXT
);

CREATE TABLE students (
    uname   TEXT REFERENCES users(uname),
    last    TEXT,
    rest    TEXT,
    teacher TEXT REFERENCES teachers(uname),
    parent  TEXT,    /* parent email address */
    fall_exam            TEXT,
    spring_exam          TEXT,
    fall_exam_fraction   REAL,
    spring_exam_fraction REAL,
    fall_notices         SMALLINT,
    spring_notices       SMALLINT
);

```

## Courses

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
    title    TEXT,     /* null should give default-generated title */
    weight   REAL     /* null should give default value of 1.0 */
);

CREATE TABLE custom_chapters (
    id    BIGSERIAL PRIMARY KEY,
    uname REFERENCES user(uname),   /* username of creator */
    title TEXT NOT NULL,
    weight REAL     /* null should give default value of 1.0 */
);

```

## Goals

```sql

CREATE TABLE goals (
    id BIGSERIAL PRIMARY KEY,
    uname   TEXT REFERENCES students(uname),
    sym     TEXT REFERENCES courses(sym), 
    custom  BIGINT REFERENCES custom_chapters(id),
    review      BOOL,
    incomplete  BOOL,
    scheduled   DATE,
    complete    DATE,
    tries INT,          /* null means 1 if complete */
    score TEXT
);

```