use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::error::AppError;
use crate::models::openai::{
    EmbeddingData, EmbeddingInput, EmbeddingRequest, EmbeddingResponse, EmbeddingUsage,
};
use crate::state::AppState;

/// Default embedding dimension (matches OpenAI's text-embedding-3-small).
const DEFAULT_DIM: usize = 1536;

/// POST /v1/embeddings
///
/// Pure-Rust embeddings using feature hashing with word unigrams,
/// bigrams, and character trigrams. No external model required.
pub async fn create_embeddings(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<EmbeddingRequest>,
) -> Result<Json<EmbeddingResponse>, AppError> {
    let dim = request.dimensions.unwrap_or(DEFAULT_DIM);
    if dim == 0 || dim > 4096 {
        return Err(AppError::BadRequest(format!(
            "dimensions must be between 1 and 4096, got {dim}"
        )));
    }

    let texts = match &request.input {
        EmbeddingInput::Single(s) => vec![s.as_str()],
        EmbeddingInput::Multiple(v) => v.iter().map(|s| s.as_str()).collect(),
    };

    let mut total_tokens = 0u32;
    let mut data = Vec::with_capacity(texts.len());

    for (i, text) in texts.iter().enumerate() {
        let tokens = approximate_token_count(text);
        total_tokens += tokens;

        let embedding = embed_text(text, dim);
        data.push(EmbeddingData {
            object: "embedding".to_string(),
            index: i as u32,
            embedding,
        });
    }

    Ok(Json(EmbeddingResponse {
        object: "list".to_string(),
        data,
        model: request.model,
        usage: EmbeddingUsage {
            prompt_tokens: total_tokens,
            total_tokens,
        },
    }))
}

/// Generate an embedding vector for the given text using feature hashing.
///
/// Combines word unigrams, word bigrams, and character trigrams
/// to capture both exact-word and sub-word similarity.
/// The result is L2-normalized to unit length.
fn embed_text(text: &str, dim: usize) -> Vec<f32> {
    let mut vec = vec![0.0f32; dim];
    let text = text.to_lowercase();

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec;
    }

    // Word unigrams (weight 1.0)
    for word in &words {
        accumulate(&mut vec, word, dim, 1.0);
    }

    // Word bigrams (weight 0.7)
    for pair in words.windows(2) {
        let bigram = format!("{} {}", pair[0], pair[1]);
        accumulate(&mut vec, &bigram, dim, 0.7);
    }

    // Character trigrams for sub-word similarity (weight 0.3)
    for word in &words {
        let chars: Vec<char> = format!("<{word}>").chars().collect();
        for tri in chars.windows(3) {
            let s: String = tri.iter().collect();
            accumulate(&mut vec, &s, dim, 0.3);
        }
    }

    // L2 normalize
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut vec {
            *x /= norm;
        }
    }

    vec
}

/// Add a hashed feature to the embedding vector using signed hashing.
fn accumulate(vec: &mut [f32], token: &str, dim: usize, weight: f32) {
    let h = fnv1a(token);
    let idx = (h as usize) % dim;
    // Use a second hash bit to determine sign (reduces collisions)
    let sign = if (h >> 32) & 1 == 0 { 1.0 } else { -1.0 };
    vec[idx] += sign * weight;
}

/// FNV-1a hash — stable across Rust versions (unlike DefaultHasher).
fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Rough token count approximation (words + punctuation).
fn approximate_token_count(text: &str) -> u32 {
    // ~1.3 tokens per whitespace-separated word (accounts for subword splits)
    let words = text.split_whitespace().count();
    (words as f32 * 1.3).ceil() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_similar_texts_have_high_cosine() {
        let a = embed_text("купить молоко в магазине", 384);
        let b = embed_text("купить молоко в магазине", 384);
        let c = embed_text("купить хлеб в магазине", 384);
        let d = embed_text("настроить сервер nginx", 384);

        let sim_same = cosine(&a, &b);
        let sim_similar = cosine(&a, &c);
        let sim_different = cosine(&a, &d);

        assert!(
            (sim_same - 1.0).abs() < 1e-5,
            "identical texts: {sim_same}"
        );
        assert!(
            sim_similar > sim_different,
            "similar ({sim_similar}) should be > different ({sim_different})"
        );
    }

    #[test]
    fn test_empty_text() {
        let v = embed_text("", 384);
        assert!(v.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn test_dimensions() {
        let v = embed_text("test", 256);
        assert_eq!(v.len(), 256);
        let v = embed_text("test", 1536);
        assert_eq!(v.len(), 1536);
    }

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }
}
