#![allow(dead_code)]
#![allow(unused_imports)]

use std::fmt::{Display, Write};

use once_cell::sync::Lazy;
use serde::Serialize;
use smallstr::SmallString;
use time::{
    Date,
    format_description::FormatItem,
    macros::format_description,
};

pub mod auth;
pub mod config;
pub mod course;
pub mod inter;
pub mod pace;
pub mod store;
pub mod user;

const DATE_FMT: &[FormatItem] = format_description!("[year]-[month]-[day]");
static EPOCH: Lazy<Date> = Lazy::new(|| 
    Date::from_calendar_date(
        1970,
        time::Month::January,
        1
    ).unwrap()
);

#[derive(Debug)]
pub enum UnifiedError {
    Postgres(tokio_postgres::error::Error),
    Auth(crate::auth::DbError),
    Data(crate::store::DbError),
    String(String),
}

impl From<tokio_postgres::error::Error> for UnifiedError {
    fn from(e: tokio_postgres::error::Error) -> Self { Self::Postgres(e) }
}
impl From<crate::auth::DbError> for UnifiedError {
    fn from(e: crate::auth::DbError) -> Self { Self::Auth(e) }
}
impl From<crate::store::DbError> for UnifiedError {
    fn from(e: crate::store::DbError) -> Self { Self::Data(e) }
}
impl From<String> for UnifiedError {
    fn from(e: String) -> Self { Self::String(e) }
}

impl Display for UnifiedError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Postgres(e) => write!(f, "Underlying database error: {}", e),
            Self::Auth(e) => write!(f, "Auth DB error: {}", e),
            Self::Data(e) => write!(f, "Data DB error: {}", e),
            Self::String(e) => write!(f, "Error: {}", e),
        }
    }
}

pub fn blank_string_means_none<S: AsRef<str>>(s: Option<S>) -> Option<S> {
    match s {
        None => None,
        Some(x) => match x.as_ref().trim() {
            "" => None,
            _ => Some(x),
        }
    }
}

pub fn now() -> time::Date {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap();
    let secs_since_epoch = since_epoch.as_secs() as i64;
    let duration_since_epoch = time::Duration::seconds(secs_since_epoch);
    EPOCH.saturating_add(duration_since_epoch)
}

#[cfg(unix)]
pub fn log_level_from_env() -> simplelog::LevelFilter {
    use simplelog::LevelFilter;

    let mut level_string = match std::env::var("LOG_LEVEL") {
        Err(_) => { return LevelFilter::Warn; },
        Ok(s) => s,
    };

    level_string.make_ascii_lowercase();
    match level_string.as_str() {
        "max" => LevelFilter::max(),
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        "off" => LevelFilter::Off,
        _ => LevelFilter::Warn,
    }
}

#[cfg(windows)]
pub fn log_level_from_env() -> simplelog::LevelFilter {
    simplelog::LevelFilter::max()
}

#[derive(Serialize)]
pub struct MiniString<A: smallvec::Array<Item = u8>>(SmallString<A>);

impl<A: smallvec::Array<Item = u8>> MiniString<A> {
    pub fn new() -> MiniString<A> {
        let inner: SmallString<A> = SmallString::new();
        MiniString(inner)
    }
}

impl<A: smallvec::Array<Item = u8>> std::ops::Deref for MiniString<A> {
    type Target = SmallString<A>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<A: smallvec::Array<Item = u8>> std::io::Write for MiniString<A> {
    fn write(&mut self, buff: &[u8]) -> std::io::Result<usize> {
        use std::io::{Error, ErrorKind};

        let str_buff = match std::str::from_utf8(buff) {
            Ok(s) => s,
            Err(_) => {
                return Err(Error::new(ErrorKind::InvalidData, "not valid UTF-8"));
            },
        };

        match self.0.write_str(str_buff) {
            Ok(()) => Ok(buff.len()),
            Err(_) => Err(Error::new(ErrorKind::Other, "formatting failed")),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn ensure_logging() {
        use simplelog::{TermLogger, TerminalMode, ColorChoice};
        let log_cfg = simplelog::ConfigBuilder::new()
            .add_filter_allow_str("camp")
            .build();
        let res = TermLogger::init(
            log_level_from_env(),
            log_cfg,
            TerminalMode::Stdout,
            ColorChoice::Auto
        );
        
        match res {
            Ok(_) => { log::info!("Test logging started."); },
            Err(_) => { log::info!("Test logging already started."); },
        }
    }
}