mod error;
mod instructions;
mod mock;
mod openai;
mod registry;
mod types;

pub use error::ProviderError;
pub use mock::MockProvider;
pub use openai::{OpenAiConfig, OpenAiProvider};
pub use registry::ProviderRegistry;
pub use types::{
    ModelSelection, Provider, ProviderId, ProviderOutput, ProviderRequest, ProviderStream,
};

pub const OPENAI_PROVIDER_ID: &str = "openai";
