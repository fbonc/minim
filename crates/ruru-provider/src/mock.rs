use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::future::Abortable;
use futures_util::future::BoxFuture;
use futures_util::{StreamExt, stream};
use ruru_types::Target;

use crate::types::RequestAbortController;
use crate::{Provider, ProviderOutput, ProviderRequest, ProviderStream};

const DEFAULT_ANSWER: &str = r"
## Epistemic Uncertainty

**Epistemic uncertainty** is uncertainty caused by a **lack of knowledge**. It arises when we do not know enough about a system, its parameters, its underlying mechanisms, or the relevant facts.

The key feature of epistemic uncertainty is that it is **potentially reducible**. By collecting more data, making better measurements, running experiments, or improving our models, we can often decrease it.

### Example

Suppose you find a coin but do not know whether it is fair. You might be uncertain whether:

$$
P(\text{heads}) = 0.5
$$

or perhaps \(0.6\), \(0.7\), or some other value.

Your uncertainty about the coin's true probability of heads is **epistemic uncertainty**. Tossing the coin many times could help you estimate that probability more accurately.

This contrasts with **aleatoric uncertainty**, which comes from inherent randomness. Even if you know with certainty that the coin is fair, you still cannot know whether the *next* toss will be heads or tails.

So, roughly:

* **Epistemic uncertainty:** *We don't know enough.*
* **Aleatoric uncertainty:** *The outcome itself is variable or random.*

### Where It Appears

Epistemic uncertainty commonly comes from:

* **Parameter uncertainty:** not knowing the exact values used in a model.
* **Model uncertainty:** not knowing whether the model itself is correct.
* **Measurement uncertainty:** imperfect observations or instruments.
* **Missing knowledge:** not knowing all relevant variables or mechanisms.

For example, a machine-learning model may have high epistemic uncertainty when it encounters data very different from anything in its training set.

### Why It Matters

Recognizing epistemic uncertainty helps prevent **false confidence**. A precise prediction is not necessarily a well-supported prediction.

It also tells us when gathering more information is valuable. If uncertainty is epistemic, additional research or evidence may substantially improve a decision.

In short:
> **Epistemic uncertainty describes what we do not know—and, importantly, what we may be able to learn.**
";

#[derive(Debug, Clone)]
pub struct MockProvider {
    chunk_delay: Duration,
    request_abort: RequestAbortController,
    last_request: Arc<Mutex<Option<(String, ProviderRequest)>>>,
}

impl MockProvider {
    pub fn new(chunk_delay: Duration) -> Self {
        Self {
            chunk_delay,
            request_abort: RequestAbortController::default(),
            last_request: Arc::default(),
        }
    }

    fn answer(request: &ProviderRequest) -> String {
        let has_image = matches!(request.target, Some(Target::Image(_)));

        match (request.prompt.as_deref(), has_image) {
            (Some(prompt), true) => format!(
                "You asked: {prompt}. Image target captured. Here is a mocked streamed reply."
            ),
            (None, true) => "Image target captured. Here is a mocked streamed reply.".to_owned(),
            (Some(prompt), false) => {
                format!("You asked: {prompt}. Here is a mocked streamed reply.")
            }
            (None, false) => DEFAULT_ANSWER.to_owned(),
        }
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new(Duration::from_millis(55))
    }
}

impl Provider for MockProvider {
    fn configured_models(&self) -> Vec<String> {
        vec!["mock".to_owned()]
    }

    fn discover_models(&self) -> BoxFuture<'_, Result<Vec<String>, crate::ProviderError>> {
        Box::pin(async { Ok(self.configured_models()) })
    }

    fn stream(&self, model: &str, request: ProviderRequest) -> ProviderStream {
        *self
            .last_request
            .lock()
            .expect("mock last request mutex poisoned") = Some((model.to_owned(), request.clone()));
        let delay = self.chunk_delay;
        let chunks = Self::answer(&request)
            .split_inclusive(' ')
            .map(|chunk| Ok(ProviderOutput::TextDelta(chunk.to_owned())))
            .collect::<Vec<_>>();

        let stream = stream::iter(chunks)
            .enumerate()
            .then(move |(index, output)| async move {
                if index > 0 && !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                output
            });

        Abortable::new(stream, self.request_abort.register()).boxed()
    }

    fn abort_request(&self) {
        self.request_abort.abort();
    }

    fn retry_request(&self) -> Option<ProviderStream> {
        let request = self
            .last_request
            .lock()
            .expect("mock last request mutex poisoned")
            .clone();

        request.map(|(model, request)| self.stream(&model, request))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_executor::block_on;
    use futures_util::StreamExt;
    use ruru_types::{ImageCapture, Target};

    use super::*;

    #[test]
    fn image_and_prompt_reach_mock_reply() {
        let request = ProviderRequest {
            prompt: Some("describe this".into()),
            target: Some(Target::Image(ImageCapture {
                png: b"png".to_vec(),
                region: None,
            })),
            context: None,
        };
        let reply = block_on(
            MockProvider::new(Duration::ZERO)
                .stream("mock", request)
                .map(Result::unwrap)
                .map(|output| match output {
                    ProviderOutput::TextDelta(chunk) => chunk,
                })
                .collect::<String>(),
        );

        assert!(reply.contains("Image target captured"));
        assert!(reply.contains("describe this"));
    }

    #[test]
    fn retries_the_last_request() {
        let provider = MockProvider::new(Duration::ZERO);
        assert!(provider.retry_request().is_none());
        let request = ProviderRequest {
            prompt: Some("retry this".into()),
            target: Some(Target::Image(ImageCapture {
                png: b"png".to_vec(),
                region: None,
            })),
            context: None,
        };
        drop(provider.stream("mock", request));

        let reply = block_on(
            provider
                .retry_request()
                .expect("stored request should be retryable")
                .map(Result::unwrap)
                .map(|output| match output {
                    ProviderOutput::TextDelta(chunk) => chunk,
                })
                .collect::<String>(),
        );

        assert!(reply.contains("Image target captured"));
        assert!(reply.contains("retry this"));
    }

    #[tokio::test]
    async fn aborts_an_active_request() {
        let provider = MockProvider::new(Duration::from_secs(60));
        let request = ProviderRequest {
            prompt: Some("describe this".into()),
            target: None,
            context: None,
        };
        let mut stream = provider.stream("mock", request);

        assert!(stream.next().await.is_some());
        provider.abort_request();

        let output = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .expect("aborted request should stop promptly");
        assert!(output.is_none());
    }
}
