use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use futures_util::stream::{self, BoxStream};
use ruru_config::{AppConfig, CredentialStore};
use ruru_provider::ProviderKind;
use ruru_types::{ModelSelection, ProviderId};

use crate::Output;
use crate::core::Core;
use crate::providers::Providers;

#[derive(Debug, Clone)]
pub struct ProviderSettingsInfo {
    pub id: ProviderId,
    pub name: &'static str,
    pub has_key: bool,
    pub models: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SettingsSnapshot {
    pub providers: Vec<ProviderSettingsInfo>,
    pub available_models: Vec<ModelSelection>,
    pub selected_model: Option<ModelSelection>,
    pub load_error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum SettingsChange {
    KeySaved,
    KeyRemoved,
    ModelsChanged,
}

#[derive(Debug, Clone)]
pub struct SettingsUpdate {
    pub provider: ProviderId,
    pub change: SettingsChange,
    pub models: Vec<String>,
    pub available_models: Vec<ModelSelection>,
}

type SaveConfig = dyn Fn(&AppConfig) -> ruru_config::Result<()> + Send + Sync;

struct SettingsState {
    config: AppConfig,
    load_error: Option<String>,
    credentials_present: BTreeSet<ProviderId>,
    latest_discovery_id: BTreeMap<ProviderId, u64>,
}

pub(crate) struct Settings {
    state: Mutex<SettingsState>,
    providers: Providers,
    save_config: Arc<SaveConfig>,
    credentials: Arc<dyn CredentialStore>,
}

impl Settings {
    pub(crate) fn load(credentials: Arc<dyn CredentialStore>) -> Self {
        Self::new(AppConfig::load(), credentials, Arc::new(AppConfig::save))
    }

    fn new(
        config_result: ruru_config::Result<AppConfig>,
        credentials: Arc<dyn CredentialStore>,
        save_config: Arc<SaveConfig>,
    ) -> Self {
        let (config, load_error) = match config_result {
            Ok(config) => (config, None),
            Err(error) => {
                eprintln!("failed to load application configuration: {error}");
                (AppConfig::default(), Some(error.to_string()))
            }
        };
        let providers = Providers::new();
        let mut credentials_present = BTreeSet::new();

        for kind in ProviderKind::ALL {
            let id = kind.id();
            match credentials.get(&id) {
                Ok(Some(key)) => {
                    let models = config
                        .providers
                        .get(&id)
                        .map(|settings| settings.models.clone())
                        .unwrap_or_default();
                    if let Err(error) = providers.configure(id.clone(), key, models) {
                        eprintln!("failed to configure {}: {error}", kind.name());
                    } else {
                        credentials_present.insert(id);
                    }
                }
                Ok(None) => {}
                Err(error) => eprintln!("failed to load {} credentials: {error}", kind.name()),
            }
        }

        Self {
            state: Mutex::new(SettingsState {
                config,
                load_error,
                credentials_present,
                latest_discovery_id: BTreeMap::new(),
            }),
            providers,
            save_config,
            credentials,
        }
    }

    pub(crate) fn providers(&self) -> Providers {
        self.providers.clone()
    }

    pub(crate) fn snapshot(&self) -> SettingsSnapshot {
        let state = self.state.lock().expect("settings mutex poisoned");
        let available_models = self.providers.configured_models();
        SettingsSnapshot {
            providers: ProviderKind::ALL
                .into_iter()
                .map(|kind| {
                    let id = kind.id();
                    ProviderSettingsInfo {
                        models: state
                            .config
                            .providers
                            .get(&id)
                            .map(|settings| settings.models.clone())
                            .unwrap_or_default(),
                        has_key: state.credentials_present.contains(&id),
                        id,
                        name: kind.name(),
                    }
                })
                .collect(),
            selected_model: state
                .config
                .selected_model
                .clone()
                .filter(|model| available_models.contains(model))
                .or_else(|| available_models.first().cloned()),
            available_models,
            load_error: state.load_error.clone(),
        }
    }

    pub(crate) fn save_key(&self, id: ProviderId, key: String) -> Result<SettingsUpdate, String> {
        let mut state = self.state.lock().expect("settings mutex poisoned");
        self.check_edit(&state, &id)?;
        let key = key.trim();
        if key.is_empty() {
            return Err("Paste an API key first".into());
        }
        let previous = self
            .credentials
            .get(&id)
            .map_err(|error| error.to_string())?;
        self.credentials
            .set(&id, key)
            .map_err(|error| error.to_string())?;
        let mut next = state.config.clone();
        next.providers.entry(id.clone()).or_default();
        let result = self.persist_provider(
            &mut state,
            id.clone(),
            next,
            Some(key.to_owned()),
            SettingsChange::KeySaved,
        );
        if result.is_err() {
            self.restore_key(&id, previous);
        } else {
            self.invalidate_discovery(&mut state, &id);
        }
        result
    }

    pub(crate) fn remove_key(&self, id: ProviderId) -> Result<SettingsUpdate, String> {
        let mut state = self.state.lock().expect("settings mutex poisoned");
        self.check_edit(&state, &id)?;
        let previous = self
            .credentials
            .get(&id)
            .map_err(|error| error.to_string())?;
        self.credentials
            .remove(&id)
            .map_err(|error| error.to_string())?;
        let next = state.config.clone();
        let result = self.persist_provider(
            &mut state,
            id.clone(),
            next,
            None,
            SettingsChange::KeyRemoved,
        );
        if result.is_err() {
            self.restore_key(&id, previous);
        } else {
            self.invalidate_discovery(&mut state, &id);
        }
        result
    }

    pub(crate) fn set_model(
        &self,
        id: ProviderId,
        model: String,
        enabled: bool,
    ) -> Result<SettingsUpdate, String> {
        let mut state = self.state.lock().expect("settings mutex poisoned");
        self.check_edit(&state, &id)?;
        let key = self
            .credentials
            .get(&id)
            .map_err(|error| error.to_string())?
            .ok_or("Save an API key first")?;
        let mut next = state.config.clone();
        let settings = next.providers.entry(id.clone()).or_default();
        if enabled {
            settings.models.push(model);
            settings.models.sort_unstable();
            settings.models.dedup();
        } else {
            settings.models.retain(|saved| saved != &model);
        }
        self.persist_provider(
            &mut state,
            id,
            next,
            Some(key),
            SettingsChange::ModelsChanged,
        )
    }

    pub(crate) fn select_model(&self, model: ModelSelection) -> Result<(), String> {
        let mut state = self.state.lock().expect("settings mutex poisoned");
        self.check_edit(&state, &model.provider)?;
        if !self.providers.configured_models().contains(&model) {
            return Err("model is not available".into());
        }
        let mut next = state.config.clone();
        next.selected_model = Some(model);
        (self.save_config)(&next).map_err(|error| error.to_string())?;
        state.config = next;
        Ok(())
    }

    pub(crate) fn discover(
        self: &Arc<Self>,
        core: Arc<Core>,
        id: ProviderId,
    ) -> BoxStream<'static, Output> {
        let request = {
            let mut state = self.state.lock().expect("settings mutex poisoned");
            self.check_edit(&state, &id).and_then(|()| {
                let key = self
                    .credentials
                    .get(&id)
                    .map_err(|error| error.to_string())?
                    .ok_or("Save an API key first")?;
                let discovery_id = state.latest_discovery_id.entry(id.clone()).or_default();
                *discovery_id = discovery_id.wrapping_add(1);
                Ok((key, *discovery_id))
            })
        };
        let (key, discovery_id) = match request {
            Ok(request) => request,
            Err(error) => {
                return stream::once(async move {
                    Output::SettingsFailed {
                        provider: Some(id),
                        error,
                    }
                })
                .boxed();
            }
        };
        let settings = Arc::clone(self);
        stream::once(async move {
            let result = Providers::discover_models(id.clone(), key.clone()).await;
            settings.finish_discovery(&core, id, discovery_id, key, result)
        })
        .flat_map(stream::iter)
        .boxed()
    }

    fn finish_discovery(
        &self,
        core: &Core,
        id: ProviderId,
        discovery_id: u64,
        key: String,
        result: Result<Vec<String>, String>,
    ) -> Vec<Output> {
        let mut state = self.state.lock().expect("settings mutex poisoned");
        if state.latest_discovery_id.get(&id) != Some(&discovery_id) {
            return Vec::new();
        }
        let Ok(models) = result else {
            return vec![Output::ModelsDiscovered {
                provider: id,
                result,
            }];
        };
        let mut outputs = Vec::new();
        let mut next = state.config.clone();
        let settings = next.providers.entry(id.clone()).or_default();
        let before = settings.models.clone();
        settings.models.retain(|model| models.contains(model));
        if settings.models != before {
            match self.persist_provider(
                &mut state,
                id.clone(),
                next,
                Some(key),
                SettingsChange::ModelsChanged,
            ) {
                Ok(update) => {
                    core.forget_provider_request();
                    outputs.push(Output::SettingsUpdated(update));
                }
                Err(error) => outputs.push(Output::SettingsFailed {
                    provider: Some(id.clone()),
                    error,
                }),
            }
        }
        outputs.push(Output::ModelsDiscovered {
            provider: id,
            result: Ok(models),
        });
        outputs
    }

    fn persist_provider(
        &self,
        state: &mut SettingsState,
        id: ProviderId,
        mut next: AppConfig,
        key: Option<String>,
        change: SettingsChange,
    ) -> Result<SettingsUpdate, String> {
        let models = next
            .providers
            .get(&id)
            .map(|settings| settings.models.clone())
            .unwrap_or_default();
        let mut available_models = self.providers.configured_models();
        available_models.retain(|selection| selection.provider != id);
        if key.is_some() {
            available_models.extend(
                models
                    .iter()
                    .map(|model| ModelSelection::new(id.clone(), model.clone())),
            );
        }
        available_models.sort();
        next.selected_model = state
            .config
            .selected_model
            .clone()
            .filter(|selected| available_models.contains(selected))
            .or_else(|| available_models.first().cloned());
        (self.save_config)(&next).map_err(|error| error.to_string())?;
        state.config = next;
        if let Some(key) = key {
            self.providers.configure(id.clone(), key, models.clone())?;
            state.credentials_present.insert(id.clone());
        } else {
            self.providers.remove(&id);
            state.credentials_present.remove(&id);
        }
        Ok(SettingsUpdate {
            provider: id,
            change,
            models,
            available_models,
        })
    }

    fn check_edit(&self, state: &SettingsState, id: &ProviderId) -> Result<(), String> {
        if let Some(error) = &state.load_error {
            return Err(format!("Cannot edit settings: {error}"));
        }
        ProviderKind::from_id(id)
            .map(|_| ())
            .ok_or_else(|| format!("unknown provider: {id}"))
    }

    fn invalidate_discovery(&self, state: &mut SettingsState, id: &ProviderId) {
        let discovery_id = state.latest_discovery_id.entry(id.clone()).or_default();
        *discovery_id = discovery_id.wrapping_add(1);
    }

    fn restore_key(&self, id: &ProviderId, previous: Option<String>) {
        let result = match previous {
            Some(key) => self.credentials.set(id, &key),
            None => self.credentials.remove(id),
        };
        if let Err(error) = result {
            eprintln!("failed to restore {id} credentials: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use ruru_config::{Error, Result};

    use super::*;

    #[derive(Default)]
    struct TestConfig {
        value: Mutex<AppConfig>,
        fail_loads: AtomicBool,
        fail_saves: AtomicBool,
    }

    impl TestConfig {
        fn load(&self) -> Result<AppConfig> {
            if self.fail_loads.load(Ordering::SeqCst) {
                return Err(Error::InvalidConfig("load failed".into()));
            }
            Ok(self.value.lock().expect("config mutex poisoned").clone())
        }

        fn save(&self, config: &AppConfig) -> Result<()> {
            if self.fail_saves.load(Ordering::SeqCst) {
                return Err(Error::InvalidConfig("save failed".into()));
            }
            *self.value.lock().expect("config mutex poisoned") = config.clone();
            Ok(())
        }
    }

    #[derive(Default)]
    struct TestCredentials(Mutex<BTreeMap<ProviderId, String>>);

    impl CredentialStore for TestCredentials {
        fn get(&self, provider: &ProviderId) -> Result<Option<String>> {
            Ok(self
                .0
                .lock()
                .expect("credentials mutex poisoned")
                .get(provider)
                .cloned())
        }

        fn set(&self, provider: &ProviderId, key: &str) -> Result<()> {
            self.0
                .lock()
                .expect("credentials mutex poisoned")
                .insert(provider.clone(), key.into());
            Ok(())
        }

        fn remove(&self, provider: &ProviderId) -> Result<()> {
            self.0
                .lock()
                .expect("credentials mutex poisoned")
                .remove(provider);
            Ok(())
        }
    }

    fn test_settings(config: Arc<TestConfig>, credentials: Arc<TestCredentials>) -> Settings {
        let loaded = config.load();
        Settings::new(loaded, credentials, Arc::new(move |next| config.save(next)))
    }

    #[test]
    fn settings_changes_update_storage_and_live_models() {
        let config = Arc::new(TestConfig::default());
        let credentials = Arc::new(TestCredentials::default());
        let settings = test_settings(config.clone(), credentials.clone());
        let id = ProviderId::new("openai");
        let model = ModelSelection::new(id.clone(), "gpt-test");

        settings.save_key(id.clone(), "test-key".into()).unwrap();
        settings
            .set_model(id.clone(), model.model.clone(), true)
            .unwrap();
        settings.select_model(model.clone()).unwrap();

        assert_eq!(settings.snapshot().available_models, vec![model.clone()]);
        assert_eq!(settings.snapshot().selected_model, Some(model));
        assert!(settings.snapshot().providers[0].has_key);

        settings.remove_key(id.clone()).unwrap();

        assert!(settings.snapshot().available_models.is_empty());
        assert!(settings.snapshot().selected_model.is_none());
        assert!(!settings.snapshot().providers[0].has_key);
        assert!(credentials.get(&id).unwrap().is_none());
        assert!(config.value.lock().unwrap().selected_model.is_none());
    }

    #[test]
    fn a_failed_config_save_restores_the_previous_key() {
        let config = Arc::new(TestConfig::default());
        let credentials = Arc::new(TestCredentials::default());
        let settings = test_settings(config.clone(), credentials.clone());
        let id = ProviderId::new("openai");
        config.fail_saves.store(true, Ordering::SeqCst);

        assert!(settings.save_key(id.clone(), "test-key".into()).is_err());
        assert!(credentials.get(&id).unwrap().is_none());
        assert!(!settings.snapshot().providers[0].has_key);
    }

    #[test]
    fn a_failed_config_load_blocks_settings_writes() {
        let config = Arc::new(TestConfig::default());
        config.fail_loads.store(true, Ordering::SeqCst);
        let credentials = Arc::new(TestCredentials::default());
        let settings = test_settings(config, credentials.clone());
        let id = ProviderId::new("openai");

        assert!(settings.snapshot().load_error.is_some());
        assert!(settings.save_key(id.clone(), "test-key".into()).is_err());
        assert!(credentials.get(&id).unwrap().is_none());
    }

    #[test]
    fn stale_discovery_cannot_remove_a_selected_model() {
        let settings = test_settings(
            Arc::new(TestConfig::default()),
            Arc::new(TestCredentials::default()),
        );
        let id = ProviderId::new("openai");
        let model = ModelSelection::new(id.clone(), "gpt-test");
        settings.save_key(id.clone(), "test-key".into()).unwrap();
        settings
            .set_model(id.clone(), model.model.clone(), true)
            .unwrap();
        settings
            .state
            .lock()
            .unwrap()
            .latest_discovery_id
            .insert(id.clone(), 2);
        let core = Core::new(
            settings.providers(),
            Arc::from(ruru_capture::new_capturer()),
        );

        let outputs = settings.finish_discovery(&core, id, 1, "old-key".into(), Ok(Vec::new()));

        assert!(outputs.is_empty());
        assert_eq!(settings.snapshot().available_models, vec![model]);
    }
}
