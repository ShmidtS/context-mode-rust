use anyhow::Result;
use std::future::Future;

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

// pub struct OllamaEmbeddingProvider {
//     host: String,
//     model: String,
// }
//
// impl OllamaEmbeddingProvider {
//     pub fn new() -> Self {
//         Self {
//             host: std::env::var("OLLAMA_HOST")
//                 .unwrap_or_else(|_| "http://localhost:11434".to_string()),
//             model: "nomic-embed-text".to_string(),
//         }
//     }
// }

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
