use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use ruru_provider::{Provider, ProviderKind};
use ruru_types::{ModelSelection, ProviderId};

#[derive(Clone, Default)]
pub struct Providers {
    providers: Arc<RwLock<HashMap<ProviderId, Arc<dyn Provider>>>>,
}

impl Providers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn configure(
        &self,
        id: ProviderId,
        key: String,
        models: Vec<String>,
    ) -> Result<(), String> {
        let kind = ProviderKind::from_id(&id).ok_or_else(|| format!("unknown provider: {id}"))?;
        self.insert(id, kind.create(key, models));
        Ok(())
    }

    pub fn remove(&self, id: &ProviderId) {
        self.providers
            .write()
            .expect("provider registry lock poisoned")
            .remove(id);
    }

    pub fn configured_models(&self) -> Vec<ModelSelection> {
        let providers = self
            .providers
            .read()
            .expect("provider registry lock poisoned")
            .iter()
            .map(|(id, provider)| (id.clone(), Arc::clone(provider)))
            .collect::<Vec<_>>();
        let mut models = providers
            .into_iter()
            .flat_map(|(id, provider)| {
                provider
                    .configured_models()
                    .into_iter()
                    .map(move |model| ModelSelection::new(id.clone(), model))
            })
            .collect::<Vec<_>>();
        models.sort();
        models
    }

    pub async fn discover_models(id: ProviderId, key: String) -> Result<Vec<String>, String> {
        let kind = ProviderKind::from_id(&id).ok_or_else(|| format!("unknown provider: {id}"))?;
        kind.create(key, Vec::new())
            .discover_models()
            .await
            .map_err(|error| error.to_string())
    }

    pub(crate) fn insert(&self, id: ProviderId, provider: Arc<dyn Provider>) {
        self.providers
            .write()
            .expect("provider registry lock poisoned")
            .insert(id, provider);
    }

    pub(crate) fn resolve(&self, selection: &ModelSelection) -> Option<Arc<dyn Provider>> {
        self.providers
            .read()
            .expect("provider registry lock poisoned")
            .get(&selection.provider)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_builds_a_configured_provider() {
        let id = ProviderKind::OpenAi.id();
        let providers = Providers::new();
        let shared = providers.clone();

        providers
            .configure(id.clone(), "test-key".into(), vec!["gpt-test".into()])
            .expect("known provider");
        assert_eq!(
            shared.configured_models(),
            vec![ModelSelection::new(id.clone(), "gpt-test")]
        );
        assert!(
            shared
                .resolve(&ModelSelection::new(id.clone(), "gpt-test"))
                .is_some()
        );

        providers.remove(&id);
        assert!(shared.configured_models().is_empty());
        assert!(
            shared
                .resolve(&ModelSelection::new(id, "gpt-test"))
                .is_none()
        );
    }

    #[test]
    fn unknown_provider_is_rejected() {
        let providers = Providers::new();
        let result = providers.configure(ProviderId::new("unknown"), "key".into(), Vec::new());
        assert!(result.is_err());
    }
}
