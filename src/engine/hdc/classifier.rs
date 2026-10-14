//! SQL one-shot classifier. A model is an ordinary, versioned BLOB value, so
//! persistence, copying, and transactions require no hidden global registry.

use super::{HvOps, Hypervector, MapVec};
use crate::engine::colbert::lightweight::{grams, hash, mix, MAX_CHARACTERS};
use crate::types::Value;
use bincode::Options;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAGIC: &[u8; 8] = b"NXHDC001";
const DIMENSION: usize = 4096;
const MAX_FEATURES: usize = 32;
const MAX_LABELS: usize = 256;
const MAX_MODEL_BYTES: u64 = 1_048_576;

#[derive(Serialize, Deserialize)]
struct Model {
    feature_count: usize,
    prototypes: BTreeMap<String, Vec<u8>>,
}

impl Model {
    fn decode(value: &Value) -> Option<Self> {
        let bytes = value.as_blob()?;
        if bytes.len() as u64 > MAX_MODEL_BYTES || !bytes.starts_with(MAGIC) {
            return None;
        }
        let model: Self = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(MAX_MODEL_BYTES)
            .reject_trailing_bytes()
            .deserialize(&bytes[MAGIC.len()..])
            .ok()?;
        if !(1..=MAX_FEATURES).contains(&model.feature_count)
            || model.prototypes.is_empty()
            || model.prototypes.len() > MAX_LABELS
        {
            return None;
        }
        for (label, bytes) in &model.prototypes {
            // Validate lengths and dimension BEFORE calling the general decoder.
            if !valid_label(label)
                || bytes.len() != 9 + DIMENSION / 8
                || bytes[0] != 1
                || bytes[1..9] != (DIMENSION as u64).to_le_bytes()
            {
                return None;
            }
        }
        Some(model)
    }

    fn encode(&self) -> Option<Value> {
        if self.prototypes.len() > MAX_LABELS {
            return None;
        }
        let mut bytes = MAGIC.to_vec();
        bytes.extend(bincode::serialize(self).ok()?);
        if bytes.len() as u64 > MAX_MODEL_BYTES {
            return None;
        }
        Some(Value::Blob(bytes))
    }
}

fn valid_label(label: &str) -> bool {
    !label.trim().is_empty() && label.chars().count() <= MAX_CHARACTERS
}

fn atom(key: &str) -> MapVec {
    let seed = hash(key);
    let data = (0..DIMENSION)
        .map(|i| {
            let bits = mix(seed.wrapping_add((i / 64) as u64));
            if bits & (1 << (i % 64)) == 0 {
                -1
            } else {
                1
            }
        })
        .collect();
    MapVec::from_raw(data)
}

/// Bind each field to its positional role, then bundle fields into one MAP
/// hypervector. Text fields use n-grams, so unseen ports retain shared structure.
fn encode_features(features: &[Value]) -> Option<Hypervector> {
    if !(1..=MAX_FEATURES).contains(&features.len()) {
        return None;
    }
    let mut fields = Vec::with_capacity(features.len());
    for (i, feature) in features.iter().enumerate() {
        let value = match feature {
            Value::Text(s) if s.chars().count() <= MAX_CHARACTERS => {
                let atoms: Vec<_> = grams(s, false)
                    .iter()
                    .map(|g| atom(&format!("text:{g}")))
                    .collect();
                MapVec::bundle(&atoms.iter().collect::<Vec<_>>()).ok()?
            }
            Value::Integer(_) | Value::Boolean(_) => {
                atom(&format!("{}:{feature}", feature.type_name()))
            }
            Value::Float(f) if f.is_finite() => {
                atom(&format!("Float:{}", if f.0 == 0.0 { 0.0 } else { f.0 }))
            }
            _ => return None,
        };
        fields.push(value.bind(&atom(&format!("field:{i}"))).ok()?);
    }
    MapVec::bundle(&fields.iter().collect::<Vec<_>>())
        .ok()
        .map(Hypervector::Map)
}

fn prediction(model: &Model, features: &[Value]) -> Option<(String, f64)> {
    if model.feature_count != features.len() {
        return None;
    }
    let query = encode_features(features)?;
    let mut scores: Vec<_> = model
        .prototypes
        .iter()
        .map(|(label, bytes)| {
            let prototype = Hypervector::from_bytes(bytes).ok()?;
            Some((label.clone(), f64::from(query.similarity(&prototype).ok()?)))
        })
        .collect::<Option<_>>()?;
    // Stable ordering over the BTreeMap gives lexical tie-breaking.
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    let best = scores.first()?;
    let runner_up = scores.get(1).map_or(0.0, |score| score.1.max(0.0));
    // Similarity margin, not a calibrated statistical probability. A tie has
    // zero confidence, and evidence below orthogonality never gives confidence.
    let confidence = (best.1.max(0.0) - runner_up).clamp(0.0, 1.0);
    Some((best.0.clone(), confidence))
}

pub(crate) fn evaluate(name: &str, args: &[Value]) -> Value {
    evaluate_inner(name, args).unwrap_or(Value::Null)
}

fn evaluate_inner(name: &str, args: &[Value]) -> Option<Value> {
    match name {
        "HDC_LEARN" if args.len() >= 3 => {
            let label = args[1].as_str()?;
            if !valid_label(label) {
                return None;
            }
            let features = &args[2..];
            let mut model = if args[0].is_null() {
                Model {
                    feature_count: features.len(),
                    prototypes: BTreeMap::new(),
                }
            } else {
                Model::decode(&args[0])?
            };
            if model.feature_count != features.len() {
                return None;
            }
            let prototype = encode_features(features)?;
            model
                .prototypes
                .insert(label.to_string(), prototype.to_bytes());
            model.encode()
        }
        "HDC_MERGE" if args.len() >= 2 => {
            let mut model = Model::decode(&args[0])?;
            for value in &args[1..] {
                let other = Model::decode(value)?;
                if model.feature_count != other.feature_count {
                    return None;
                }
                model.prototypes.extend(other.prototypes);
            }
            model.encode()
        }
        "HDC_PREDICT" | "HDC_CONFIDENCE" if args.len() >= 2 => {
            let model = Model::decode(&args[0])?;
            let (label, confidence) = prediction(&model, &args[1..])?;
            Some(if name == "HDC_PREDICT" {
                Value::Text(label)
            } else {
                Value::from(confidence)
            })
        }
        _ => None,
    }
}
