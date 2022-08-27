/*!
Displaying individual student calendars.
*/
use smallstr::SmallString;
use time::{
    Date,
    format_description::FormatItem,
    macros::format_description,
};

use crate::{
    user::Student,
};

use super::*;

const DATE_FMT: &[FormatItem] = format_description!("[month repr:short] [day]");

static TEMP_RESPONSE: &str = r#"<!doctype html>
<html>
<head>
    <meta charset="utf-8">
    <title>CAMP | Unimplemented</title>
</head>
<body>
    <h1>Student login as of yet unimplemented.</h1>
    <p>Please come back at a later date.</p>
</body>
</html>"#;

pub async fn login(
    s: Student,
    form: LoginData,
    glob: Arc<RwLock<Glob>>
) -> Response {

    let glob = glob.read().await;
    match glob.auth().read().await.check_password(
        &s.base.uname,
        &form.password,
        &s.base.salt
    ).await {
        Err(e) => {
            log::error!(
                "auth::Db::check_password( {:?}, {:?}, {:?} ) error: {}",
                &s.base.uname, &form.password, &s.base.salt, &e
            );
            return html_500();
        },
        Ok(AuthResult::Ok) => { /* This is the happy path; proceed. */ },
        Ok(AuthResult::BadPassword) => { return respond_bad_password(&s.base.uname); },
        Ok(x) => {
            log::warn!(
                "auth::Db::check_password( {:?}, {:?}, {:?} ) returned {:?}, which shouldn't happen.",
                &s.base.uname, &form.password, &s.base.salt, &x
            );
            return respond_bad_password(&s.base.uname);
        },
    }

    (
        StatusCode::NOT_IMPLEMENTED,
        Html(TEMP_RESPONSE)
    ).into_response()
}