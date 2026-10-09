use std::sync::Arc;

use crate::{OPENAI_PROVIDER_ID, OpenAiConfig, OpenAiProvider, Provider, ProviderId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAi,
}

impl ProviderKind {
    pub const ALL: [Self; 1] = [Self::OpenAi];

    pub fn from_id(id: &ProviderId) -> Option<Self> {
        match id.as_str() {
            OPENAI_PROVIDER_ID => Some(Self::OpenAi),
            _ => None,
        }
    }

    pub fn id(self) -> ProviderId {
        match self {
            Self::OpenAi => ProviderId::new(OPENAI_PROVIDER_ID),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
        }
    }

    pub fn create(self, key: String, models: Vec<String>) -> Arc<dyn Provider> {
        match self {
            Self::OpenAi => Arc::new(OpenAiProvider::new(OpenAiConfig::new(key, models))),
        }
    }
}
