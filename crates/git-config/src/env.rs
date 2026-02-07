//! Environment variable overrides for git configuration.

use bstr::BString;

use crate::error::ConfigError;
use crate::{ConfigEntry, ConfigKey, ConfigScope};

/// Load configuration overrides from environment variables.
///
/// Supports the GIT_CONFIG_COUNT / GIT_CONFIG_KEY_N / GIT_CONFIG_VALUE_N protocol.
pub fn load_env_overrides() -> Result<Vec<ConfigEntry>, ConfigError> {
    let mut entries = Vec::new();

    // GIT_CONFIG_COUNT / KEY / VALUE
    if let Ok(count_str) = std::env::var("GIT_CONFIG_COUNT") {
        let count: usize = count_str
            .parse()
            .map_err(|_| ConfigError::InvalidInt(format!("GIT_CONFIG_COUNT={}", count_str)))?;

        for i in 0..count {
            let key_env = format!("GIT_CONFIG_KEY_{}", i);
            let value_env = format!("GIT_CONFIG_VALUE_{}", i);

            let key = std::env::var(&key_env).map_err(|_| {
                ConfigError::InvalidKey(format!("{} not set (GIT_CONFIG_COUNT={})", key_env, count))
            })?;

            let value = std::env::var(&value_env).unwrap_or_default();

            let config_key = ConfigKey::parse(&key)?;
            entries.push(ConfigEntry {
                key: config_key,
                value: Some(BString::from(value.as_bytes())),
                scope: ConfigScope::Command,
                source_file: None,
                line_number: None,
            });
        }
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Environment variable tests need to be run serially since they
    // modify global state. In practice, these are tested via integration tests
    // that set env vars in a subprocess.

    #[test]
    fn load_empty_env() {
        // When GIT_CONFIG_COUNT is not set, should return empty
        // (This test assumes the env var is not set in the test environment)
        std::env::remove_var("GIT_CONFIG_COUNT");
        let entries = load_env_overrides().unwrap();
        assert!(entries.is_empty());
    }
}
