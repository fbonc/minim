use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use ruru_provider::{ModelSelection, ProviderId};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::{Error, Result};

const CONFIG_FILE: &str = "config.toml";
const CONFIG_DIRECTORY: &str = "ruru";
const LEGACY_CONFIG_DIRECTORIES: [&str; 2] = ["minim", "little-owl"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    pub selected_model: Option<ModelSelection>,
    pub providers: BTreeMap<ProviderId, ProviderSettings>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            selected_model: None,
            providers: BTreeMap::new(),
        }
    }
}

impl AppConfig {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn load() -> Result<Self> {
        let legacy_paths = LEGACY_CONFIG_DIRECTORIES
            .map(Self::path_for)
            .into_iter()
            .collect::<Result<Vec<_>>>()?;
        Self::load_from_paths(Self::path()?, &legacy_paths)
    }

    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let config = Self::default();
                config.save_to(path)?;
                return Ok(config);
            }
            Err(error) => return Err(error.into()),
        };
        let config = toml::from_str::<Self>(&contents)
            .map_err(|error| Error::InvalidConfig(error.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(Self::path()?)
    }

    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<()> {
        self.validate()?;
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        if let Some(parent) = parent {
            fs::create_dir_all(parent)?;
        }
        let directory = parent.unwrap_or_else(|| Path::new("."));
        let contents = toml::to_string_pretty(self)
            .map_err(|error| Error::InvalidConfig(error.to_string()))?;
        let mut temporary = NamedTempFile::new_in(directory)?;
        temporary.write_all(contents.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }

    pub fn path() -> Result<PathBuf> {
        Self::path_for(CONFIG_DIRECTORY)
    }

    fn path_for(directory: &str) -> Result<PathBuf> {
        let base_dirs = BaseDirs::new().ok_or(Error::ConfigDirectoryUnavailable)?;
        Ok(base_dirs.config_dir().join(directory).join(CONFIG_FILE))
    }

    fn load_from_paths(path: PathBuf, legacy_paths: &[PathBuf]) -> Result<Self> {
        if path.exists() {
            return Self::load_from(path);
        }

        for legacy_path in legacy_paths {
            if legacy_path.exists() {
                let config = Self::load_from(legacy_path)?;
                config.save_to(path)?;
                return Ok(config);
            }
        }

        Self::load_from(path)
    }

    fn validate(&self) -> Result<()> {
        if self.version != Self::CURRENT_VERSION {
            return Err(Error::UnsupportedVersion(self.version));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderSettings {
    pub enabled: bool,
    pub models: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configured() -> AppConfig {
        AppConfig {
            selected_model: Some(ModelSelection::new(ProviderId::new("openai"), "gpt-test")),
            providers: BTreeMap::from([(
                ProviderId::new("openai"),
                ProviderSettings {
                    enabled: true,
                    models: vec!["gpt-test".into()],
                },
            )]),
            ..AppConfig::default()
        }
    }

    #[test]
    fn missing_file_creates_the_default_configuration() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("nested/config.toml");

        let config = AppConfig::load_from(&path).expect("missing config should create defaults");

        assert_eq!(config, AppConfig::default());
        assert!(path.is_file());
        assert_eq!(
            AppConfig::load_from(path).expect("load created config"),
            AppConfig::default()
        );
    }

    #[test]
    fn configuration_round_trips_through_toml() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("nested/config.toml");
        let expected = configured();

        expected.save_to(&path).expect("save config");
        let actual = AppConfig::load_from(path).expect("load config");

        assert_eq!(actual, expected);
    }

    #[test]
    fn legacy_configuration_is_copied_to_the_current_path() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let current_path = directory.path().join("ruru/config.toml");
        let minim_path = directory.path().join("minim/config.toml");
        let oldest_path = directory.path().join("little-owl/config.toml");
        let expected = configured();
        expected.save_to(&minim_path).expect("save legacy config");
        AppConfig::default()
            .save_to(&oldest_path)
            .expect("save older config");

        let actual = AppConfig::load_from_paths(current_path.clone(), &[minim_path, oldest_path])
            .expect("migrate legacy config");

        assert_eq!(actual, expected);
        assert_eq!(
            AppConfig::load_from(current_path).expect("load migrated config"),
            expected
        );
    }

    #[test]
    fn oldest_configuration_is_used_when_newer_legacy_path_is_missing() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let current_path = directory.path().join("ruru/config.toml");
        let minim_path = directory.path().join("minim/config.toml");
        let oldest_path = directory.path().join("little-owl/config.toml");
        let expected = configured();
        expected.save_to(&oldest_path).expect("save oldest config");

        let actual = AppConfig::load_from_paths(current_path.clone(), &[minim_path, oldest_path])
            .expect("migrate oldest config");

        assert_eq!(actual, expected);
        assert_eq!(
            AppConfig::load_from(current_path).expect("load migrated config"),
            expected
        );
    }

    #[test]
    fn saving_replaces_an_existing_configuration() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("config.toml");
        fs::write(&path, "old contents").expect("seed config");
        let expected = configured();

        expected.save_to(&path).expect("replace config");

        assert_eq!(AppConfig::load_from(path).expect("load config"), expected);
    }

    #[test]
    fn malformed_configuration_is_not_replaced() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("config.toml");
        let malformed = "this is not = valid toml";
        fs::write(&path, malformed).expect("seed config");

        assert!(matches!(
            AppConfig::load_from(&path),
            Err(Error::InvalidConfig(_))
        ));
        assert_eq!(fs::read_to_string(path).expect("read config"), malformed);
    }

    #[test]
    fn unsupported_configuration_version_is_rejected() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("config.toml");
        let config = AppConfig {
            version: AppConfig::CURRENT_VERSION + 1,
            ..AppConfig::default()
        };
        let contents = toml::to_string(&config).expect("serialize config");
        fs::write(&path, contents).expect("seed config");

        assert!(matches!(
            AppConfig::load_from(path),
            Err(Error::UnsupportedVersion(version))
                if version == AppConfig::CURRENT_VERSION + 1
        ));
    }

    #[test]
    fn provider_models_default_to_empty_for_existing_configuration() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("config.toml");
        fs::write(
            &path,
            r#"
version = 1

[providers.openai]
enabled = true
"#,
        )
        .expect("seed config");

        let config = AppConfig::load_from(path).expect("load config");

        assert_eq!(
            config.providers[&ProviderId::new("openai")].models,
            Vec::<String>::new()
        );
    }

    #[test]
    fn configuration_contains_no_credential_field() {
        let serialized = toml::to_string(&configured()).expect("serialize config");

        assert!(!serialized.contains("api_key"));
        assert!(!serialized.contains("credential"));
    }
}
