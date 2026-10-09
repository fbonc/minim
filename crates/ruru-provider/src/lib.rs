mod error;
mod instructions;
mod kind;
mod mock;
mod openai;
mod types;

pub use error::ProviderError;
pub use kind::ProviderKind;
pub use mock::MockProvider;
pub use openai::{OpenAiConfig, OpenAiProvider};
pub use ruru_types::{ModelSelection, ProviderId};
pub use types::{Provider, ProviderOutput, ProviderRequest, ProviderStream};

pub const OPENAI_PROVIDER_ID: &str = "openai";
