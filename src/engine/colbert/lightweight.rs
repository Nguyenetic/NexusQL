//! Character n-gram token embeddings with ColBERT's MaxSim late interaction.
//! Intended for short identifiers and labels, without a tokenizer or model file.

use super::maxsim::maxsim_score;
use ndarray::Array2;

const DIMENSION: usize = 64;
pub const MAX_CHARACTERS: usize = 256;

/// Fixed hash and mixing algorithm: unlike DefaultHasher or a library RNG,
/// these token vectors remain stable across processes and dependency upgrades.
pub(crate) fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}

pub(crate) fn mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

pub(crate) fn grams(text: &str, punctuation: bool) -> Vec<String> {
    let chars: Vec<_> = text
        .to_lowercase()
        .chars()
        .filter(|c| punctuation || c.is_alphanumeric())
        .collect();
    if chars.is_empty() {
        return vec![];
    }
    let chars: Vec<_> = std::iter::once('^')
        .chain(chars)
        .chain(std::iter::once('$'))
        .collect();
    [2, 3]
        .into_iter()
        .flat_map(|size| {
            chars
                .windows(size)
                .map(|window| window.iter().collect::<String>())
        })
        .collect()
}

fn embeddings(text: &str, punctuation: bool) -> Array2<f32> {
    let grams = grams(text, punctuation);
    let mut embeddings = Array2::zeros((grams.len(), DIMENSION));
    for (token, gram) in grams.iter().enumerate() {
        let bits = mix(hash(gram));
        for coordinate in 0..DIMENSION {
            // 64 coordinates of +/- 1/8 have unit length.
            embeddings[[token, coordinate]] = if bits & (1 << coordinate) == 0 {
                -0.125
            } else {
                0.125
            };
        }
    }
    embeddings
}

fn normalized_maxsim(query: &Array2<f32>, candidate: &Array2<f32>) -> f64 {
    if query.nrows() == 0 || candidate.nrows() == 0 {
        return 0.0;
    }
    f64::from((maxsim_score(query, candidate) / query.nrows() as f32).clamp(0.0, 1.0))
}

/// Bounded [0,1] lexical relevance. Most weight ignores punctuation; the small
/// literal component ranks an exact device name above its punctuation variant.
/// This is not a learned semantic embedding or a calibrated probability.
pub fn score(query: &str, candidate: &str) -> Option<f64> {
    if query.chars().count() > MAX_CHARACTERS || candidate.chars().count() > MAX_CHARACTERS {
        return None;
    }
    if query.trim().is_empty() || candidate.trim().is_empty() {
        return Some(0.0);
    }
    let normalized = normalized_maxsim(&embeddings(query, false), &embeddings(candidate, false));
    let literal = normalized_maxsim(&embeddings(query, true), &embeddings(candidate, true));
    Some(0.9 * normalized + 0.1 * literal)
}
