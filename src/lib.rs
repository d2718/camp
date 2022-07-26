#![allow(dead_code)]
#![allow(unused_imports)]

pub mod auth;
pub mod course;

#[cfg(test)]
mod tests {
    pub fn ensure_logging() {
        use simplelog::{TermLogger, TerminalMode, ColorChoice, LevelFilter};
        let log_cfg = simplelog::ConfigBuilder::new()
            .add_filter_allow_str("camp")
            .build();
        let res = TermLogger::init(
            LevelFilter::max(),
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