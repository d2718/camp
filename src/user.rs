/*!
Database users.
*/
use std::io::Read;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    Admin,
    Boss,
    Teacher,
    Student,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use std::fmt::Write;

        let token = match self {
            Role::Admin   => "Admin",
            Role::Boss    => "Boss",
            Role::Teacher => "Teacher",
            Role::Student => "Student",
        };

        write!(f, "{}", token)
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Admin"   => Ok(Role::Admin),
            "Boss"    => Ok(Role::Boss),
            "Teacher" => Ok(Role::Teacher),
            "Student" => Ok(Role::Student),
            _ => Err(format!("{:?} is not a valid Role.", s)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BaseUser {
    pub uname: String,
    pub role: Role,
    pub salt: String,
    pub email: String,
}

impl BaseUser {
    pub fn into_admin(self) -> User { User::Admin(self) }
    pub fn into_boss(self) -> User { User::Boss(self) }
    pub fn into_teacher(self, name: String) -> User {
        User::Teacher(Teacher { base: self, name })
    }
    pub fn into_student(
        self,
        last: String,
        rest: String,
        teacher: String,
        parent: String,
        fall_exam: Option<String>,
        spring_exam: Option<String>,
        fall_exam_fraction: f32,
        spring_exam_fraction: f32,
        fall_notices: i16,
        spring_notices: i16,
    ) -> User {
        let s = Student {
            base: self, last, rest, teacher, parent,
            fall_exam, spring_exam,
            fall_exam_fraction, spring_exam_fraction,
            fall_notices, spring_notices,
        };
        User::Student(s)
    }
}

#[derive(Debug)]
pub struct Teacher {
    pub base: BaseUser,
    pub name: String,
}

#[derive(Debug)]
pub struct Student {
    pub base:BaseUser,
    /// Last name of the student.
    pub last: String,
    /// The rest of the student's name (first, middle initial, etc.).
    pub rest: String,
    /// `uname` of the student's teacher.
    pub teacher: String,
    /// Parent email address(es? if possible?).
    pub parent: String,
    pub fall_exam: Option<String>,
    pub spring_exam: Option<String>,
    pub fall_exam_fraction: f32,
    pub spring_exam_fraction: f32,
    pub fall_notices: i16,
    pub spring_notices: i16,
}

impl Student {
    /**
    Student .csv rows should look like this

    ```csv
    #uname, last,   rest, email,                    parent,                 teacher
    jsmith, Smith,  John, lil.j.smithy@gmail.com,   js.senior@gmail.com,    jenny
    ```
    */
    pub fn from_csv_line(
        row: &csv::StringRecord
    ) -> Result<Student, &'static str> {
        log::trace!("Student::from_csv_line( {:?} ) called.", row);

        let uname = match row.get(0) {
            Some(s) => s.to_owned(),
            None => { return Err("no uname"); }
        };
        let email = match row.get(3) {
            Some(s) => s.to_owned(),
            None => { return Err("no email address"); },
        };
        
        let base = BaseUser {
            uname,
            role: Role::Student,
            salt: String::new(),
            email,
        };

        let last = match row.get(1) {
            Some(s) => s.to_owned(),
            None => { return Err("no last name"); },
        };
        let rest = match row.get(2) {
            Some(s) => s.to_owned(),
            None => { return Err("no rest of name"); },
        };
        let teacher = match row.get(5) {
            Some(s) => s.to_owned(),
            None => { return Err("no teacher uname"); },
        };
        let parent = match row.get(4) {
            Some(s) => s.to_owned(),
            None => { return Err("no parent email"); },
        };
        
        let stud = Student {
            base,
            last,
            rest,
            teacher,
            parent,
            fall_exam: None,
            spring_exam: None,
            fall_exam_fraction: 0.2_f32,
            spring_exam_fraction: 0.2_f32,
            fall_notices: 0,
            spring_notices: 0,
        };
        Ok(stud)
    }

    pub fn vec_from_csv_reader<R: Read>(r: R) -> Result<Vec<Student>, String> {
        log::trace!("Student::vec_from_csv_reader(...) called.");

        let mut csv_reader = csv::ReaderBuilder::new()
            .comment(Some(b'#'))
            .trim(csv::Trim::All)
            .flexible(false)
            .has_headers(false)
            .from_reader(r);
        
        // We overestimate the amount of `Student`s required and then
        // shrink it later.
        let mut students: Vec<Student> = Vec::with_capacity(256);

        for (n, res) in csv_reader.records().enumerate() {
            match res {
                Ok(record) => match Student::from_csv_line(&record) {
                    Ok(stud) => { students.push(stud); },
                    Err(e) => {
                        let estr = match record.position() {
                            Some(p) => format!(
                                "Error on line {}: {}",
                                p.line(), &e
                            ),
                            None => format!(
                                "Error in CSV record {}: {}", &n, &e
                            ),
                        };
                        return Err(estr);
                    },
                },
                Err(e) => {
                    let estr = match e.position() {
                        Some(p) => format!(
                            "Error on line {}: {}", p.line(), &e
                        ),
                        None => format!(
                            "Error in CSV record {}: {}", &n, &e
                        ),
                    };
                    return Err(estr);
                }
            }
        }

        students.shrink_to_fit();
        log::trace!(
            "Students::vec_from_csv_reader() returns {} Students.",
            students.len()
        );
        Ok(students)
    }
}

#[derive(Debug)]
pub enum User {
    Admin(BaseUser),
    Boss(BaseUser),
    Teacher(Teacher),
    Student(Student),
}

impl User {
    pub fn uname(&self) -> &str {
        match self {
            User::Admin(base) => &base.uname,
            User::Boss(base) => &base.uname,
            User::Teacher(t) => &t.base.uname,
            User::Student(s) => &s.base.uname,
        }
    }

    pub fn salt(&self) -> &str {
        match self {
            User::Admin(base) => &base.salt,
            User::Boss(base) => &base.salt,
            User::Teacher(t) => &t.base.salt,
            User::Student(s) => &s.base.salt,
        }
    }

    pub fn email(&self) -> &str {
        match self {
            User::Admin(base) => &base.email,
            User::Boss(base) => &base.email,
            User::Teacher(t) => &t.base.email,
            User::Student(s) => &s.base.email,
        }
    }

    pub fn role(&self) -> Role {
        match self {
            User::Admin(_) => Role::Admin,
            User::Boss(_) => Role::Boss,
            User::Teacher(_) => Role::Teacher,
            User::Student(_) => Role::Student,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::ensure_logging;

    #[test]
    fn students_from_csv() {
        ensure_logging();
        let f = std::fs::File::open("test/good_students_0.csv").unwrap();
        let studs = Student::vec_from_csv_reader(f).unwrap();
        log::trace!("Students:\n{:#?}", &studs);
    }
}