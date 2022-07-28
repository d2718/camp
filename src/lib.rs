#![allow(dead_code)]
#![allow(unused_imports)]

pub mod auth;
pub mod course;
pub mod store;

fn log_level_from_env() -> simplelog::LevelFilter {
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