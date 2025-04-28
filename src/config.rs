//! Configuration management for imgen.
//!
//! Handles loading and saving user configuration, primarily the OpenAI API key,
//! from a platform-standard location (`~/.config/imgen/config.json` on Linux/macOS,
//! `%APPDATA%\imgen\config.json` on Windows).

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    env,
    error::Error,
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

const CONFIG_FILE_NAME: &str = "config.json";
const APPLICATION: &str = "imgen";

/// Represents the user configuration.
#[derive(Serialize, Deserialize, Default)]
#[cfg_attr(test, derive(Debug, Clone, PartialEq, Eq))]
pub struct Config {
    /// The user's OpenAI API key.
    pub openai_api_key: Option<String>,
}

/// Errors that can occur during configuration loading or saving.
#[derive(Debug)]
pub enum ConfigError {
    /// Could not determine configuration location
    NoConfig,
    /// I/O error accessing config file
    Io(io::Error),
    /// Failed to deserialize config file
    Deserialize(serde_json::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NoConfig => {
                write!(f, "Could not determine configuration location")
            }
            ConfigError::Io(err) => {
                write!(f, "I/O error accessing config file: {err}")
            }
            ConfigError::Deserialize(err) => {
                write!(f, "Failed to deserialize config file: {err}")
            }
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::Io(err) => Some(err),
            ConfigError::Deserialize(err) => Some(err),
            ConfigError::NoConfig => None,
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> Self {
        ConfigError::Io(err)
    }
}

/// Gets the platform-specific path to the configuration directory.
///
/// Returns `None` if the config directory cannot be determined.
fn config_dir() -> Option<PathBuf> {
    let mut dir =
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME").map(|home| {
                    let mut path = PathBuf::from(home);
                    path.push(".config");
                    path
                })
            })?;

    dir.push(APPLICATION);
    Some(dir)
}

/// Gets the platform-specific path to the configuration file.
///
/// Returns `None` if the config path cannot be determined.
fn config_path() -> Option<PathBuf> {
    let mut path = config_dir()?;
    path.push(CONFIG_FILE_NAME);
    Some(path)
}

impl Config {
    /// Loads the configuration from the default location.
    ///
    /// If the config file does not exist or cannot be read/parsed,
    /// a default `Config` is returned and a warning is logged.
    pub fn load() -> Config {
        let config_path = match config_path() {
            Some(path) => path,
            None => return Config::default(),
        };

        match Config::load_from_path(&config_path) {
            Ok(config) => {
                debug!("Config loaded from: {}", config_path.display());
                config
            }
            Err(ConfigError::NoConfig) => Config::default(),
            Err(err) => {
                warn!(
                    "Failed to load config from {}: {err}",
                    config_path.display()
                );
                Config::default()
            }
        }
    }

    /// Tries to load the configuration from a specific path.
    pub fn load_from_path(path: &Path) -> Result<Config, ConfigError> {
        debug!("Attempting to load config from: {}", path.display());
        let contents = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(ref err) if err.kind() == io::ErrorKind::NotFound => {
                return Err(ConfigError::NoConfig)
            }
            Err(err) => {
                return Err(ConfigError::Io(err));
            }
        };
        serde_json::from_str::<Config>(&contents)
            .map_err(ConfigError::Deserialize)
    }

    /// Saves the configuration to the default location.
    ///
    /// Creates the configuration directory if it doesn't exist.
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = config_path().ok_or(ConfigError::NoConfig)?;
        self.save_to_path(&path)
    }

    /// Saves the configuration to a specific path.
    ///
    /// Creates the parent directory if it doesn't exist.
    pub fn save_to_path(&self, path: &Path) -> Result<(), ConfigError> {
        debug!("Attempting to save config to: {}", path.display());
        if let Some(parent_dir) = path.parent() {
            // Create directory if needed
            fs::create_dir_all(parent_dir)?;
        }

        // Panic on serialization error since that should never happen.
        let contents = serde_json::to_string_pretty(self)
            .expect("Failed to serialize config");

        let mut file_opts = fs::OpenOptions::new();
        file_opts.write(true).create(true);

        // The config contains secrets, so set permissions to -rw--------
        #[cfg(unix)]
        file_opts.mode(0o600);

        // Write the config to the file
        let mut file = file_opts.open(path)?;
        file.write_all(contents.as_bytes())?;

        info!("Config saved to: {}", path.display());
        Ok(())
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    // Helper to create a config path within a temporary directory
    fn temp_config_path(temp_dir: &tempfile::TempDir) -> PathBuf {
        temp_dir.path().join(CONFIG_FILE_NAME)
    }

    #[test]
    fn test_get_config_path_returns_some() {
        let path = config_path().expect("Config path should be Some");
        assert!(path.ends_with(Path::new(APPLICATION).join(CONFIG_FILE_NAME)));
    }

    #[test]
    fn test_load_config_non_existent() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_config_path(&temp_dir);
        assert!(!config_path.exists());

        // Loading non-existent file should return NoConfig error
        let result = Config::load_from_path(&config_path);
        assert!(matches!(result, Err(ConfigError::NoConfig)));
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_config_path(&temp_dir);

        let original_config = Config {
            openai_api_key: Some("test-api-key-123".to_string()),
        };

        // Save the config
        original_config.save_to_path(&config_path).unwrap();

        // Ensure the file was created
        assert!(config_path.exists());

        // Verify permissions on Unix
        #[cfg(unix)]
        {
            let metadata = fs::metadata(&config_path).unwrap();
            let permissions = metadata.permissions();
            assert_eq!(
                permissions.mode() & 0o777,
                0o600,
                "Permissions should be 0o600"
            );
        }

        // Load the config back
        let loaded_config = Config::load_from_path(&config_path).unwrap();

        // Verify the loaded config matches the original
        assert_eq!(loaded_config, original_config);
    }
}
