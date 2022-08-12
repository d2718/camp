#![allow(dead_code)]
#![allow(unused_imports)]

use std::fmt::{Display, Write};

use time::{
    Date,
    format_description::FormatItem,
    macros::format_description,
};

pub mod auth;
pub mod config;
pub mod course;
pub mod inter;
pub mod store;
pub mod user;

const DATE_FMT: &[FormatItem] = format_description!("[year]-[month]-[day]");

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