/*!
Populating the local "fake production" environment with sufficient data
to allow some experimentation.

Fake production data can be found in `crate_root/fakeprod_data`.
*/
use std::{
    fs::File,
    io::Read,
    path::Path,
};

use simplelog::{ColorChoice, TerminalMode, TermLogger};

use camp::*;
use camp::{
    course::{Course},
    user::{BaseUser, Role, Student, Teacher, User},
};

/**
CSV file format:

(Unlike all the other types of CSVs we use in production, this file
DOES have a header.)

```csv
role, uname, email, name
a,    root,  root@not.an.email
b,    boss,  boss@our.system.com
t,    jenny, jenny@camelotacademy.org, Jenny Feaster
t,    irfan, irfan@camelotacademy.org, Irfan Azam
# ... etc
```
*/
fn csv_file_to_staff<R: Read>(r: R) -> Result<Vec<User>, String> {
    log::trace!("csv_file_to_staff( ... ) called.");

    let mut csv_reader = csv::ReaderBuilder::new()
        .comment(Some(b'#'))
        .trim(csv::Trim::All)
        .flexible(true)
        .has_headers(true)
        .from_reader(r);
    
    let mut users: Vec<User> = Vec::new();

    for (n, res) in csv_reader.records().enumerate() {
        let rec = res.map_err(|e| format!(
            "Error in CSV record {}: {}", &n, &e
        ))?;
        let role = match rec.get(0)
            .ok_or_else(|| format!("Line {}: no role.", &n))? {
            "a" | "A" => Role::Admin,
            "b" | "B" => Role::Boss,
            "t" | "T" => Role::Teacher,
            x => { return Err(format!(
                "Line {}: unrecognized role: {:?}", &n, &x
            )); },
        };

        let uname = rec.get(1)
            .ok_or_else(|| format!("Line {}: no uname.", &n))?
            .to_owned();
        let email = rec.get(2)
            .ok_or_else(|| format!("Line {}: no email.", &n))?
            .to_owned();
        
        let bu = BaseUser { uname, role, email, salt: String::new() };
        let u = match role {
            Role::Admin => bu.into_admin(),
            Role::Boss => bu.into_boss(),
            Role::Teacher => {
                let name = rec.get(3)
                    .ok_or_else(|| format!(
                        "Line {}: no name for teacher.", &n
                    ))?;
                bu.into_teacher(name.to_owned())
            }
            Role::Student => { return Err(format!(
                "Line {} should not contain a student.", &n
            )); },
        };

        users.push(u);
    }

    Ok(users)
}

fn read_course_dir<P: AsRef<Path>>(p: P) -> Result<Vec<Course>, String> {
    let p = p.as_ref();
    log::trace!("read_course_dir( {} ) called.", p.display());

    let mut courses: Vec<Course> = Vec::new();

    for res in std::fs::read_dir(p)
        .map_err(|e| format!(
            "Error reading course dir {}: {}", p.display(), &e
        ))?
    {
        let ent = match res {
            Ok(ent) => ent,
            Err(e) => {
                log::warn!(
                    "Error reading directory entry: {}", &e
                );
                continue;
            }
        };

        let path = ent.path();
        if path.extension() != Some("mix".as_ref()) {
            log::info!(
                "Skipping file without \".mix\" extension in cours dir: {}",
                &path.display()
            );
            continue;
        }

        let f = File::open(&path)
            .map_err(|e| format!(
                "Error opening course file {}: {}", p.display(), &e
            ))?;
        
        let course = Course::from_reader(f)
            .map_err(|e| format!(
                "Error reading course file {}: {}", p.display(), &e
            ))?;
        
        courses.push(course);
    }
    
    Ok(courses)
}

#[tokio::main(flavor = "current_thread")]
    async fn main() -> Result<(), UnifiedError> {
        let log_cfg = simplelog::ConfigBuilder::new()
        .add_filter_allow_str("camp")
        .build();
    TermLogger::init(
        camp::log_level_from_env(),
        log_cfg,
        TerminalMode::Stdout,
        ColorChoice::Auto
    ).unwrap();
    log::info!("Logging started.");

    Ok(())
}