use std::sync::Arc;

use base64::Engine;
use eventsource_stream::Eventsource;
use futures_util::future::Abortable;
use futures_util::{StreamExt, stream};
use minim_types::{ImageCapture, Target};
use serde::Serialize;
use serde_json::Value;

use crate::types::RequestAbortController;
use crate::{Provider, ProviderError, ProviderOutput, ProviderRequest, ProviderStream};

const DEFAULT_BASE_URL: &str = "https://api.openai.com";
use super::instructions::INSTRUCTIONS;

pub struct OpenAiConfig {
    pub api_key: String,
    pub models: Vec<String>,
    pub base_url: String,
}

impl OpenAiConfig {
    pub fn new(api_key: impl Into<String>, models: Vec<String>) -> Self {
        Self {
            api_key: api_key.into(),
            models,
            base_url: DEFAULT_BASE_URL.into(),
        }
    }
}

#[derive(Clone)]
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: Arc<str>,
    models: Vec<String>,
    endpoint: Arc<str>,
    request_abort: RequestAbortController,
}

impl OpenAiProvider {
    pub fn new(config: OpenAiConfig) -> Self {
        let endpoint = format!("{}/v1/responses", config.base_url.trim_end_matches('/'));

        Self {
            client: reqwest::Client::new(),
            api_key: config.api_key.into(),
            models: config.models,
            endpoint: endpoint.into(),
            request_abort: RequestAbortController::default(),
        }
    }

    async fn open_stream(
        client: reqwest::Client,
        api_key: Arc<str>,
        endpoint: Arc<str>,
        request: ResponsesRequest,
    ) -> Result<ProviderStream, ProviderError> {
        let response = client
            .post(endpoint.as_ref())
            .bearer_auth(api_key.as_ref())
            .json(&request)
            .send()
            .await
            .map_err(|error| ProviderError::new(format!("OpenAI request failed: {error}")))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.map_err(|error| {
                ProviderError::new(format!("failed to read OpenAI error response: {error}"))
            })?;
            return Err(api_error(status, &body));
        }

        Ok(response
            .bytes_stream()
            .eventsource()
            .filter_map(|event| async move {
                match event {
                    Ok(event) => parse_event(&event.data),
                    Err(error) => Some(Err(ProviderError::new(format!(
                        "OpenAI response stream failed: {error}"
                    )))),
                }
            })
            .boxed())
    }
}

impl Provider for OpenAiProvider {
    fn available_models(&self) -> Vec<String> {
        self.models.clone()
    }

    fn stream(&self, model: &str, request: ProviderRequest) -> ProviderStream {
        let client = self.client.clone();
        let api_key = Arc::clone(&self.api_key);
        let endpoint = Arc::clone(&self.endpoint);
        let request = ResponsesRequest::new(model, request);

        let stream = stream::once(async move {
            match Self::open_stream(client, api_key, endpoint, request).await {
                Ok(stream) => stream,
                Err(error) => stream::once(async move { Err(error) }).boxed(),
            }
        })
        .flatten();

        Abortable::new(stream, self.request_abort.register()).boxed()
    }

    fn abort_request(&self) {
        self.request_abort.abort();
    }
}

#[derive(Serialize)]
struct ResponsesRequest {
    model: String,
    instructions: &'static str,
    input: Vec<InputMessage>,
    stream: bool,
    store: bool,
}

impl ResponsesRequest {
    fn new(model: &str, request: ProviderRequest) -> Self {
        let mut content = Vec::new();

        if let Some(context) = request.context {
            content.push(InputContent::Text {
                text: "Surrounding context image:".into(),
            });
            content.push(InputContent::from_image(context.image));
            if let Some(text) = context.accessibility_text {
                content.push(InputContent::Text {
                    text: format!("Supplemental accessibility context:\n{text}"),
                });
            }
        }

        if let Some(target) = request.target {
            match target {
                Target::Text(target) => content.push(InputContent::Text {
                    text: format!("Target:\n{}", target.text),
                }),
                Target::Image(image) => {
                    content.push(InputContent::Text {
                        text: "Target image:".into(),
                    });
                    content.push(InputContent::from_image(image));
                }
            }
        }

        content.push(InputContent::Text {
            text: format!("Question:\n{}", request.prompt.as_deref().unwrap_or("")),
        });

        Self {
            model: model.into(),
            instructions: INSTRUCTIONS,
            input: vec![InputMessage {
                role: "user",
                content,
            }],
            stream: true,
            store: false,
        }
    }
}

#[derive(Serialize)]
struct InputMessage {
    role: &'static str,
    content: Vec<InputContent>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum InputContent {
    #[serde(rename = "input_text")]
    Text { text: String },
    #[serde(rename = "input_image")]
    Image {
        image_url: String,
        detail: &'static str,
    },
}

impl InputContent {
    fn from_image(image: ImageCapture) -> Self {
        let encoded = base64::engine::general_purpose::STANDARD.encode(image.png);
        Self::Image {
            image_url: format!("data:image/png;base64,{encoded}"),
            detail: "auto",
        }
    }
}

fn parse_event(data: &str) -> Option<Result<ProviderOutput, ProviderError>> {
    if data.is_empty() || data == "[DONE]" {
        return None;
    }

    let event = match serde_json::from_str::<Value>(data) {
        Ok(event) => event,
        Err(error) => {
            return Some(Err(ProviderError::new(format!(
                "invalid OpenAI response event: {error}"
            ))));
        }
    };

    match event.get("type").and_then(Value::as_str) {
        Some("response.output_text.delta" | "response.refusal.delta") => {
            match event.get("delta").and_then(Value::as_str) {
                Some(delta) => Some(Ok(ProviderOutput::TextDelta(delta.into()))),
                None => Some(Err(ProviderError::new(
                    "OpenAI text delta did not contain text",
                ))),
            }
        }
        Some("error") => Some(Err(ProviderError::new(
            event
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("OpenAI returned an unknown streaming error"),
        ))),
        Some("response.failed") => Some(Err(ProviderError::new(
            event
                .pointer("/response/error/message")
                .and_then(Value::as_str)
                .unwrap_or("OpenAI response failed"),
        ))),
        _ => None,
    }
}

fn api_error(status: reqwest::StatusCode, body: &str) -> ProviderError {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|body| {
            body.pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown API error".into());

    ProviderError::new(format!("OpenAI API request failed ({status}): {message}"))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::StreamExt;
    use minim_types::{ContextCapture, ImageCapture, Target, TextCapture, TextCaptureMethod};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    fn request(target: Option<Target>) -> ProviderRequest {
        ProviderRequest {
            prompt: Some("What does this mean?".into()),
            target,
            context: Some(ContextCapture {
                image: ImageCapture {
                    png: b"context".to_vec(),
                    region: None,
                },
                accessibility_text: Some("surrounding text".into()),
            }),
        }
    }

    #[test]
    fn builds_request_with_image_context_for_responses_api() {
        let request = ResponsesRequest::new(
            "gpt-test",
            request(Some(Target::Text(TextCapture {
                text: "selected text".into(),
                method: TextCaptureMethod::Accessibility,
            }))),
        );
        let json = serde_json::to_value(request).expect("serialize request");

        assert_eq!(json["model"], "gpt-test");
        assert_eq!(json["stream"], true);
        assert_eq!(json["store"], false);
        assert_eq!(json["input"][0]["role"], "user");
        assert_eq!(
            json["input"][0]["content"][0]["text"],
            "Surrounding context image:"
        );
        assert_eq!(json["input"][0]["content"][1]["type"], "input_image");
        assert_eq!(
            json["input"][0]["content"][2]["text"],
            "Supplemental accessibility context:\nsurrounding text"
        );
        assert_eq!(
            json["input"][0]["content"][3]["text"],
            "Target:\nselected text"
        );
        assert_eq!(
            json["input"][0]["content"][4]["text"],
            "Question:\nWhat does this mean?"
        );
    }

    #[test]
    fn encodes_png_target_as_an_input_image() {
        let request = ResponsesRequest::new(
            "gpt-test",
            request(Some(Target::Image(ImageCapture {
                png: b"png".to_vec(),
                region: None,
            }))),
        );
        let json = serde_json::to_value(request).expect("serialize request");
        let image = &json["input"][0]["content"][4];

        assert_eq!(image["type"], "input_image");
        assert_eq!(image["image_url"], "data:image/png;base64,cG5n");
        assert_eq!(image["detail"], "auto");
    }

    #[test]
    fn parses_text_and_error_events() {
        assert_eq!(
            parse_event(r#"{"type":"response.output_text.delta","delta":"hello"}"#),
            Some(Ok(ProviderOutput::TextDelta("hello".into())))
        );
        assert_eq!(
            parse_event(r#"{"type":"error","message":"rate limited"}"#),
            Some(Err(ProviderError::new("rate limited")))
        );
        assert_eq!(
            parse_event(r#"{"type":"response.refusal.delta","delta":"I cannot"}"#),
            Some(Ok(ProviderOutput::TextDelta("I cannot".into())))
        );
        assert_eq!(parse_event(r#"{"type":"response.completed"}"#), None);
    }

    #[tokio::test]
    async fn streams_text_deltas_from_the_responses_endpoint() {
        let events = concat!(
            "data: {\"type\":\"response.created\"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hello \"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"owl\"}\n\n",
            "data: {\"type\":\"response.completed\"}\n\n"
        );
        let (base_url, received) = serve_once(200, "text/event-stream", events).await;
        let provider = OpenAiProvider::new(OpenAiConfig {
            api_key: "test-key".into(),
            models: vec!["gpt-test".into()],
            base_url,
        });

        let output = provider
            .stream("gpt-test", request(None))
            .map(Result::unwrap)
            .map(|output| match output {
                ProviderOutput::TextDelta(delta) => delta,
            })
            .collect::<String>()
            .await;
        let received = received.await.expect("server task");

        assert_eq!(output, "hello owl");
        assert!(received.starts_with("POST /v1/responses HTTP/1.1"));
        assert!(
            received
                .to_ascii_lowercase()
                .contains("authorization: bearer test-key")
        );
    }

    #[tokio::test]
    async fn returns_api_error_messages() {
        let body = r#"{"error":{"message":"invalid API key"}}"#;
        let (base_url, received) = serve_once(401, "application/json", body).await;
        let provider = OpenAiProvider::new(OpenAiConfig {
            api_key: "bad-key".into(),
            models: vec!["gpt-test".into()],
            base_url,
        });

        let error = provider
            .stream("gpt-test", request(None))
            .next()
            .await
            .expect("provider result")
            .expect_err("request should fail");
        received.await.expect("server task");

        assert_eq!(
            error,
            ProviderError::new("OpenAI API request failed (401 Unauthorized): invalid API key")
        );
    }

    #[tokio::test]
    async fn aborts_an_active_request() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        let provider = OpenAiProvider::new(OpenAiConfig {
            api_key: "test-key".into(),
            models: vec!["gpt-test".into()],
            base_url: format!("http://{address}"),
        });
        let mut stream = provider.stream("gpt-test", request(None));
        let next_output = tokio::spawn(async move { stream.next().await });
        let _connection = tokio::time::timeout(Duration::from_secs(1), listener.accept())
            .await
            .expect("request should reach test server")
            .expect("accept request");

        provider.abort_request();

        let output = tokio::time::timeout(Duration::from_secs(1), next_output)
            .await
            .expect("aborted request should stop promptly")
            .expect("stream task");
        assert!(output.is_none());
    }

    async fn serve_once(
        status: u16,
        content_type: &'static str,
        response_body: &'static str,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept request");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];

            loop {
                let count = socket.read(&mut buffer).await.expect("read request");
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);

                let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + 4 + content_length {
                    break;
                }
            }

            let reason = if status == 200 { "OK" } else { "Error" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                response_body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
            String::from_utf8(request).expect("UTF-8 request")
        });

        (format!("http://{address}"), task)
    }
}
