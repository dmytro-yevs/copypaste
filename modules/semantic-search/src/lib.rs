//! Offline embeddings. Only the installed module links its inference runtime.
#![deny(unsafe_code)]

use copypaste_module_sdk::{
    validate_embeddings, Module, ModuleEnvironment, ModuleInvocation, ModuleOutput,
    ModuleSearchProvider, SearchModel,
};
use ort::{
    session::{builder::GraphOptimizationLevel, Session},
    value::Tensor,
};
use std::{collections::BTreeMap, fs, path::PathBuf};
use tokenizers::{Tokenizer, TruncationParams};

pub struct SemanticSearchModule {
    environment: ModuleEnvironment,
    provider: ModuleSearchProvider,
    engine: Option<Engine>,
}

impl Module for SemanticSearchModule {
    fn create(environment: ModuleEnvironment) -> Result<Self, String> {
        let bytes =
            fs::read(PathBuf::from(&environment.package_dir).join("assets/search-models.json"))
                .map_err(|_| "The semantic search module is incomplete.")?;
        let provider: ModuleSearchProvider =
            copypaste_module_sdk::serde_json::from_slice(&bytes)
                .map_err(|_| "The semantic search model configuration is invalid.")?;
        for model in &provider.models {
            model.validate()?;
        }
        Ok(Self {
            environment,
            provider,
            engine: None,
        })
    }

    fn invoke(&mut self, invocation: ModuleInvocation) -> Result<ModuleOutput, String> {
        if invocation.command != self.provider.command {
            return Err("Unknown semantic search command.".into());
        }
        let text = invocation
            .arguments
            .get("text")
            .and_then(|value| value.as_str())
            .filter(|text| !text.is_empty() && text.len() <= 64 * 1024)
            .ok_or("A bounded search text is required.")?;
        let role = invocation
            .arguments
            .get("role")
            .and_then(|value| value.as_str())
            .ok_or("The embedding role is required.")?;
        if !matches!(role, "query" | "passage") {
            return Err("The embedding role is invalid.".into());
        }
        let languages = invocation
            .preferences
            .get(&self.provider.language_field)
            .and_then(|value| value.as_array())
            .ok_or("Choose at least one search language.")?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or("The search language is invalid.")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let model = self
            .provider
            .model_for(&languages)
            .ok_or("No model supports the selected languages.")?;
        if self
            .engine
            .as_ref()
            .is_none_or(|engine| engine.model.id != model.id)
        {
            self.engine = Some(Engine::load(&self.environment, model.clone())?);
        }
        let vectors = self
            .engine
            .as_mut()
            .ok_or("The search model is unavailable.")?
            .encode(text, role)?;
        if !validate_embeddings(&vectors, model.dimensions) {
            return Err("The search model returned invalid embeddings.".into());
        }
        Ok(ModuleOutput::Embeddings {
            model_id: model.id.clone(),
            vectors,
        })
    }
}

struct Engine {
    model: SearchModel,
    tokenizer: Tokenizer,
    session: Session,
}

impl Engine {
    fn load(environment: &ModuleEnvironment, model: SearchModel) -> Result<Self, String> {
        tokenizers::utils::parallelism::set_parallelism(false);
        let runtime_name = if cfg!(target_os = "windows") {
            "onnxruntime.dll"
        } else if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.so"
        };
        let runtime = PathBuf::from(&environment.package_dir)
            .join("bin")
            .join(runtime_name);
        ort::init_from(&runtime)
            .map_err(|_| "The packaged search runtime could not be loaded.")?
            .with_telemetry(false)
            .commit();
        let directory = PathBuf::from(&environment.data_dir)
            .join("models")
            .join(model.resource_directory_name());
        let mut tokenizer = Tokenizer::from_file(directory.join("tokenizer.json"))
            .map_err(|_| "The search tokenizer could not be loaded. Download its model again.")?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: model.max_tokens,
                stride: 64,
                ..Default::default()
            }))
            .map_err(|_| "The search tokenizer configuration is invalid.")?;
        tokenizer.with_padding(None);
        let session = Session::builder()
            .map_err(model_failed)?
            .with_intra_threads(2)
            .map_err(model_failed)?
            .with_inter_threads(1)
            .map_err(model_failed)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(model_failed)?
            .commit_from_file(directory.join("model.onnx"))
            .map_err(model_failed)?;
        Ok(Self {
            model,
            tokenizer,
            session,
        })
    }

    fn encode(&mut self, text: &str, role: &str) -> Result<Vec<Vec<f32>>, String> {
        let prefix = if role == "query" {
            &self.model.query_prefix
        } else {
            &self.model.passage_prefix
        };
        let prefix = self
            .tokenizer
            .encode(prefix.as_str(), false)
            .map_err(|_| "The search prefix could not be tokenized.")?;
        self.tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: self.model.max_tokens.saturating_sub(prefix.len()),
                stride: 64,
                ..Default::default()
            }))
            .map_err(|_| "The search tokenizer configuration is invalid.")?;
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|_| "The search text could not be tokenized.")?;
        let mut encodings = vec![&encoding];
        if role == "passage" {
            encodings.extend(encoding.get_overflowing());
        }
        if encodings.len() > 128 {
            return Err("The embedding fragment exceeds its chunk limit.".into());
        }
        let mut vectors = Vec::with_capacity(encodings.len());
        for encoding in encodings {
            let mut ids: Vec<i64> = encoding.get_ids().iter().map(|id| i64::from(*id)).collect();
            let mut mask: Vec<i64> = encoding
                .get_attention_mask()
                .iter()
                .map(|mask| i64::from(*mask))
                .collect();
            let mut types: Vec<i64> = encoding
                .get_type_ids()
                .iter()
                .map(|id| i64::from(*id))
                .collect();
            // Every overflow passage receives the retrieval prefix. Reserving
            // its tokens before truncation preserves all passage text.
            if !prefix.is_empty() {
                ids.splice(1..1, prefix.get_ids().iter().map(|id| i64::from(*id)));
                mask.splice(1..1, std::iter::repeat_n(1, prefix.len()));
                types.splice(1..1, std::iter::repeat_n(0, prefix.len()));
            }
            let length = ids.len();
            let mut inputs = BTreeMap::from([
                (
                    "input_ids",
                    Tensor::from_array(([1, length], ids)).map_err(model_failed)?,
                ),
                (
                    "attention_mask",
                    Tensor::from_array(([1, length], mask.clone())).map_err(model_failed)?,
                ),
            ]);
            if self
                .session
                .inputs()
                .iter()
                .any(|input| input.name() == "token_type_ids")
            {
                inputs.insert(
                    "token_type_ids",
                    Tensor::from_array(([1, length], types)).map_err(model_failed)?,
                );
            }
            let outputs = self
                .session
                .run(inputs.into_iter().collect::<Vec<_>>())
                .map_err(model_failed)?;
            let (shape, values) = outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(model_failed)?;
            if shape.len() != 3
                || shape[0] != 1
                || shape[1] != length as i64
                || shape[2] != self.model.dimensions as i64
            {
                return Err("The search model output shape is invalid.".into());
            }
            vectors.push(mean_pool_normalized(values, &mask, self.model.dimensions)?);
        }
        Ok(vectors)
    }
}

fn mean_pool_normalized(
    values: &[f32],
    mask: &[i64],
    dimensions: usize,
) -> Result<Vec<f32>, String> {
    if dimensions == 0 || values.len() != mask.len() * dimensions {
        return Err("The search tensor is invalid.".into());
    }
    let mut vector = vec![0.0f32; dimensions];
    let mut count = 0.0f32;
    for (token, mask) in values.chunks_exact(dimensions).zip(mask) {
        if *mask == 0 {
            continue;
        }
        count += 1.0;
        for (target, value) in vector.iter_mut().zip(token) {
            *target += value;
        }
    }
    if count == 0.0 {
        return Err("The search tensor has no text tokens.".into());
    }
    for value in &mut vector {
        *value /= count;
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= f32::EPSILON {
        return Err("The search vector is invalid.".into());
    }
    for value in &mut vector {
        *value /= norm;
    }
    Ok(vector)
}

fn model_failed<E>(_: E) -> String {
    // Runtime diagnostics can contain user-owned model paths; never expose them.
    "The semantic search model could not run. Download its model again.".into()
}

copypaste_module_sdk::export_module!(SemanticSearchModule);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pools_only_visible_tokens_and_normalizes() {
        let vector =
            mean_pool_normalized(&[3.0, 0.0, 0.0, 4.0, 99.0, 99.0], &[1, 1, 0], 2).unwrap();
        assert!((vector[0] - 0.6).abs() < 0.001);
        assert!((vector[1] - 0.8).abs() < 0.001);
        assert!(mean_pool_normalized(&[f32::NAN, 0.0], &[1], 2).is_err());
    }
}
