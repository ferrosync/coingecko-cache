use std::env;
use log::{warn, error};

pub fn init_logging(default_filters: &str) {
    let log_env_raw = env::var("RUST_LOG");
    let log_env = log_env_raw.clone().ok()
        .filter(|env| !env.is_empty())
        .unwrap_or(default_filters.into());

    pretty_env_logger::formatted_timed_builder()
        .parse_filters(&log_env)
        .init();

    match &log_env_raw {
        Err(env::VarError::NotUnicode(..)) =>
            error!("Failed to read 'RUST_LOG' due to invalid Unicode. Using default instead: '{}'", default_filters),

        Err(env::VarError::NotPresent) =>
            warn!("Missing 'RUST_LOG'. Using default instead: '{}'", default_filters),

        Ok(s) if s.is_empty() =>
            warn!("Got empty 'RUST_LOG'. Using default instead: '{}'", default_filters),

        Ok(_) => (),
    }
}
