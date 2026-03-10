use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct SanctifyConfig {
    pub kani: Option<KaniSettings>,
}

#[derive(Debug, Deserialize)]
pub struct KaniSettings {
    pub unwind: Option<u32>,
}

pub fn load_config() -> Result<SanctifyConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(".sanctify.toml")?;
    let config: SanctifyConfig = toml::from_str(&content)?;
    Ok(config)
}
