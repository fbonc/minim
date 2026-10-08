use std::sync::Arc;

use ruru_config::{AppConfig, CredentialStore, KeychainCredentialStore};
use ruru_provider::{
    ModelSelection, OPENAI_PROVIDER_ID, OpenAiConfig, OpenAiProvider, ProviderId, ProviderRegistry,
};

pub(super) fn load_providers() -> (ProviderRegistry, Option<ModelSelection>) {
    let config = AppConfig::load().unwrap_or_else(|error| {
        eprintln!("failed to load application configuration: {error}");
        AppConfig::default()
    });
    let providers = providers_from_config(&config, &KeychainCredentialStore::default());
    let configured_models = providers.configured_models();
    let selected_model = config
        .selected_model
        .filter(|model| configured_models.contains(model))
        .or_else(|| configured_models.into_iter().next());

    (providers, selected_model)
}

fn providers_from_config(
    config: &AppConfig,
    credentials: &dyn CredentialStore,
) -> ProviderRegistry {
    let providers = ProviderRegistry::new();
    let provider_id = ProviderId::new(OPENAI_PROVIDER_ID);
    let Some(settings) = config
        .providers
        .get(&provider_id)
        .filter(|settings| settings.enabled)
    else {
        return providers;
    };

    match credentials.get(&provider_id) {
        Ok(Some(api_key)) => {
            providers.add_provider(
                provider_id,
                Arc::new(OpenAiProvider::new(OpenAiConfig::new(
                    api_key,
                    settings.models.clone(),
                ))),
            );
        }
        Ok(None) => eprintln!("OpenAI is enabled but has no stored API key"),
        Err(error) => eprintln!("failed to load the OpenAI API key: {error}"),
    }

    providers
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ruru_config::{ProviderSettings, Result};

    use super::*;

    struct TestCredentials(Option<String>);

    impl CredentialStore for TestCredentials {
        fn get(&self, _provider: &ProviderId) -> Result<Option<String>> {
            Ok(self.0.clone())
        }

        fn set(&self, _provider: &ProviderId, _api_key: &str) -> Result<()> {
            Ok(())
        }

        fn remove(&self, _provider: &ProviderId) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn registers_enabled_openai_provider_with_stored_credentials() {
        let provider_id = ProviderId::new(OPENAI_PROVIDER_ID);
        let config = AppConfig {
            providers: BTreeMap::from([(
                provider_id.clone(),
                ProviderSettings {
                    enabled: true,
                    models: vec!["gpt-test".into()],
                },
            )]),
            ..AppConfig::default()
        };

        let providers = providers_from_config(&config, &TestCredentials(Some("test-key".into())));

        assert_eq!(
            providers.configured_models(),
            vec![ModelSelection::new(provider_id, "gpt-test")]
        );
    }

    #[test]
    fn skips_openai_provider_without_stored_credentials() {
        let config = AppConfig {
            providers: BTreeMap::from([(
                ProviderId::new(OPENAI_PROVIDER_ID),
                ProviderSettings {
                    enabled: true,
                    models: vec!["gpt-test".into()],
                },
            )]),
            ..AppConfig::default()
        };

        let providers = providers_from_config(&config, &TestCredentials(None));

        assert!(providers.configured_models().is_empty());
    }
}
