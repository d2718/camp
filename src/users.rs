/*!
Database users.
*/

pub enum Role {
    Admin,
    Boss,
    Teacher,
    Student,
}

pub struct User {
    uname: String,
    role: Role,
    salt: String,
    email: String,
}

pub struct Teacher {
    user: User,
    name: String,
}

pub struct Student {
    user: User,
    /// Last name of the student.
    last: String,
    /// The rest of the student's name (first, middle initial, etc.).
    rest: String,
    /// `uname` of the student's teacher.
    teacher: String,
    /// Parent email address(es? if possible?).
    parent: String,
}

