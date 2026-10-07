use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::ModelSelection;
use super::Provider;
use super::ProviderId;

#[derive(Clone, Default)]
pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<ProviderId, Arc<dyn Provider>>>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_provider(
        &self,
        id: ProviderId,
        provider: Arc<dyn Provider>,
    ) -> Option<Arc<dyn Provider>> {
        self.providers
            .write()
            .expect("provider registry lock poisoned")
            .insert(id, provider)
    }

    pub fn remove_provider(&self, id: &ProviderId) -> Option<Arc<dyn Provider>> {
        self.providers
            .write()
            .expect("provider registry lock poisoned")
            .remove(id)
    }

    pub fn resolve(&self, selection: &ModelSelection) -> Option<Arc<dyn Provider>> {
        self.providers
            .read()
            .expect("provider registry lock poisoned")
            .get(&selection.provider)
            .cloned()
    }

    pub fn available_models(&self) -> Vec<ModelSelection> {
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
                    .available_models()
                    .into_iter()
                    .map(move |model| ModelSelection::new(id.clone(), model))
            })
            .collect::<Vec<_>>();
        models.sort();
        models
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::MockProvider;

    use super::*;

    #[test]
    fn lists_and_resolves_provider_models() {
        let registry = ProviderRegistry::new();
        let shared_registry = registry.clone();
        let provider_id = ProviderId::new("mock");
        registry.add_provider(
            provider_id.clone(),
            Arc::new(MockProvider::new(Duration::ZERO)),
        );

        let models = shared_registry.available_models();

        assert_eq!(
            models,
            vec![ModelSelection::new(provider_id.clone(), "mock")]
        );
        assert!(shared_registry.resolve(&models[0]).is_some());
        registry.remove_provider(&provider_id);
        assert!(shared_registry.available_models().is_empty());
        assert!(shared_registry.resolve(&models[0]).is_none());
    }
}
