use log::info;
use domfi_util::{ConfigError, ConfigContext};

pub struct Config {
    pub agent_name: String,
    pub postgres_url: String,
}

pub async fn config_with_prefix(prefix: &str) -> Result<Config, ConfigError> {
    let config = ConfigContext::new(prefix);

    let agent_name = config.var("AGENT_NAME").unwrap_or("loader_historical".into());
    info!("Agent name: '{}'", agent_name);

    let postgres_url = config.var("POSTGRES_URL")?;

    Ok(Config {
        agent_name,
        postgres_url,
    })
}
