use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::Duration;

pub const MODEL_DIM: usize = 384;

pub trait EmbeddingProvider {
    fn embed(&self, texts: &[String]) -> impl Future<Output = Result<Vec<Vec<f32>>>> + Send;

    fn dim(&self) -> usize;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ZeroEmbeddingProvider;

impl ZeroEmbeddingProvider {
    pub fn new() -> Self {
        Self
    }
}

impl EmbeddingProvider for ZeroEmbeddingProvider {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.0; MODEL_DIM]).collect())
    }

    fn dim(&self) -> usize {
        MODEL_DIM
    }
}

#[derive(Debug, Clone)]
pub struct OllamaEmbeddingProvider {
    host: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct OllamaEmbeddingRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

impl OllamaEmbeddingProvider {
    pub fn new() -> Self {
        Self {
            host: std::env::var("OLLAMA_HOST")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            model: std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "nomic-embed-text".to_string()),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    fn embeddings_url(&self) -> String {
        format!("{}/api/embeddings", self.host.trim_end_matches('/'))
    }

    fn zero_embedding(&self) -> Vec<f32> {
        vec![0.0; MODEL_DIM]
    }

    async fn embed_one(&self, text: &str) -> Result<Vec<f32>, reqwest::Error> {
        let request = OllamaEmbeddingRequest {
            model: &self.model,
            prompt: text,
        };
        let response: OllamaEmbeddingResponse = self
            .client
            .post(self.embeddings_url())
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut embedding = response.embedding;
        embedding.resize(MODEL_DIM, 0.0);
        embedding.truncate(MODEL_DIM);
        Ok(embedding)
    }
}

impl Default for OllamaEmbeddingProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingProvider for OllamaEmbeddingProvider {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            match self.embed_one(text).await {
                Ok(embedding) => embeddings.push(embedding),
                Err(error) => {
                    tracing::warn!(
                        "Ollama embedding unavailable, using zero embedding: {}",
                        error
                    );
                    embeddings.push(self.zero_embedding());
                }
            }
        }
        Ok(embeddings)
    }

    fn dim(&self) -> usize {
        MODEL_DIM
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct NoopWaker;

    impl Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::from(Arc::new(NoopWaker));
        let mut context = Context::from_waker(&waker);
        let mut future = pin!(future);

        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[test]
    fn test_zero_provider_dim() {
        let provider = ZeroEmbeddingProvider::new();

        assert_eq!(provider.dim(), MODEL_DIM);
    }

    #[test]
    fn test_zero_provider_returns_zeros() {
        let provider = ZeroEmbeddingProvider::new();
        let texts = vec!["one".to_string(), "two".to_string()];

        let embeddings = block_on(provider.embed(&texts)).unwrap();

        assert_eq!(embeddings.len(), 2);
        assert!(
            embeddings
                .iter()
                .all(|embedding| embedding.len() == MODEL_DIM)
        );
        assert!(embeddings.iter().flatten().all(|value| *value == 0.0));
    }
}
