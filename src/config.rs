#![allow(dead_code)]

use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct Config {
    pub endpoint: Option<String>,
    pub model: Option<String>,
    pub api_key_env_var: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct TomlConfig {
    pub provider: Option<Config>,
}

pub(crate) fn load_config(app_name: &str, defaults: Config) -> Config {
    let prefix = app_name.to_uppercase().replace("-", "_");
    let mut merged = defaults;
    if let Some(path) = config_path(app_name) {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(toml) = toml::from_str::<TomlConfig>(&content) {
                if let Some(p) = toml.provider {
                    if p.endpoint.is_some() { merged.endpoint = p.endpoint; }
                    if p.model.is_some() { merged.model = p.model; }
                    if p.api_key_env_var.is_some() { merged.api_key_env_var = p.api_key_env_var; }
                }
            }
        }
    }
    let env = |k: &str| env::var(format!("{}_{}", prefix, k)).ok();
    if let Some(v) = env("ENDPOINT") { merged.endpoint = Some(v); }
    if let Some(v) = env("MODEL") { merged.model = Some(v); }
    if let Some(v) = env("API_KEY_ENV") { merged.api_key_env_var = Some(v); }
    merged
}

pub(crate) fn config_path(app_name: &str) -> Option<PathBuf> {
    let base = env::var("XDG_CONFIG_HOME")
        .or_else(|_| env::var("HOME"))
        .map(PathBuf::from)
        .ok()?;
    Some(base.join(".config").join(app_name).join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config_uses_defaults() {
        let defaults = Config {
            endpoint: Some("http://localhost:11434".into()),
            model: Some("llama3".into()),
            api_key_env_var: None,
        };
        let cfg = load_config("vibe-tape-test-unlikely-name", defaults.clone());
        assert_eq!(cfg.endpoint, defaults.endpoint);
        assert_eq!(cfg.model, defaults.model);
    }
}
