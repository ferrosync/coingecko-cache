pub mod tcp;

// Re-export trait for chaining multiple errors
pub use multi_try::MultiTry;

use std::env;
use log::{warn, error};
use snafu::{Snafu, ResultExt};

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

#[derive(Debug, Snafu)]
pub enum ConfigError {
    #[snafu(display("'{}' missing or unset in '.env' file: {}", name, source))]
    BadVariable {
        name: String,
        source: env::VarError,
    },

    #[snafu(display("{} errors occurred attempting to read config: {:?}", errors.len(), errors))]
    ErrorCollection {
        errors: Vec<ConfigError>,
    }
}

pub trait IntoConfigResult<T> {
    fn into_config_result(self) -> Result<T, ConfigError>;
}

impl<T> IntoConfigResult<T> for Result<T, Vec<ConfigError>> {
    fn into_config_result(self) -> Result<T, ConfigError> {
        self.map_err(|e| ConfigError::ErrorCollection { errors: e })
    }
}

pub struct ConfigContext {
    prefix: String,
}

impl ConfigContext {
    pub fn new(prefix: impl AsRef<str>) -> ConfigContext {
        ConfigContext {
            prefix: prefix.as_ref().to_owned(),
        }
    }

    pub fn name_of(&self, name: impl AsRef<str>) -> String {
        format!("{}_{}", self.prefix, name.as_ref())
    }

    pub fn var(&self, name: impl AsRef<str>) -> Result<String, ConfigError> {
        env::var(self.name_of(name.as_ref()))
            .context(BadVariable { name: name.as_ref().to_owned() })
    }
}
