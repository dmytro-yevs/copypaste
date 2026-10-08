//! Signed model resources and the smallest contract for search embeddings.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleSearchProvider {
    pub command: String,
    pub language_field: String,
    pub models: Vec<SearchModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SearchModel {
    pub id: String,
    pub title: String,
    pub languages: Vec<String>,
    pub dimensions: usize,
    pub max_tokens: usize,
    pub query_prefix: String,
    pub passage_prefix: String,
    pub minimum_similarity: f32,
    pub files: Vec<SearchModelFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SearchModelFile {
    pub path: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

impl SearchModel {
    pub fn resource_directory_name(&self) -> String {
        use sha2::Digest;
        let mut inventory: Vec<_> = self
            .files
            .iter()
            .map(|file| (&file.path, &file.sha256, file.size_bytes))
            .collect();
        inventory.sort_by_key(|file| file.0);
        let hash = sha2::Sha256::digest(serde_json::to_vec(&inventory).unwrap_or_default());
        format!("{}-{}", self.id, hex::encode(&hash[..8]))
    }
    pub fn size_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.size_bytes).sum()
    }

    pub fn validate(&self) -> Result<(), String> {
        if !crate::valid_id(&self.id)
            || self.title.trim().is_empty()
            || self.title.len() > 160
            || self.languages.is_empty()
            || self.languages.len() > 128
            || !(1..=4096).contains(&self.dimensions)
            || !(16..=512).contains(&self.max_tokens)
            || !self.minimum_similarity.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_similarity)
            || self.query_prefix.len() > 64
            || self.passage_prefix.len() > 64
            || self.files.is_empty()
            || self.files.len() > 16
        {
            return Err("The search model is invalid.".into());
        }
        let mut paths = std::collections::BTreeSet::new();
        let mut languages = std::collections::BTreeSet::new();
        if self
            .languages
            .iter()
            .any(|language| !crate::valid_id(language) || !languages.insert(language))
        {
            return Err("The search model languages are invalid.".into());
        }
        for file in &self.files {
            let revision = file
                .url
                .split("/resolve/")
                .nth(1)
                .and_then(|path| path.split('/').next());
            if !crate::valid_relative_path(&file.path)
                || !paths.insert(&file.path)
                || !file.url.starts_with("https://huggingface.co/")
                || file.url.len() > 2048
                || !revision.is_some_and(|revision| {
                    revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                || file.url.contains(['?', '#', '\\'])
                || file.sha256.len() != 64
                || !file.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                || file.size_bytes == 0
                || file.size_bytes > 512 * 1024 * 1024
            {
                return Err("The search model file inventory is invalid.".into());
            }
        }
        if !paths.contains(&"model.onnx".to_owned())
            || !paths.contains(&"tokenizer.json".to_owned())
        {
            return Err("The search model is incomplete.".into());
        }
        Ok(())
    }
}

impl ModuleSearchProvider {
    /// Smallest compatible profile; selecting more languages does not download
    /// several models when one profile covers the complete selection.
    pub fn model_for(&self, languages: &[String]) -> Option<&SearchModel> {
        if languages.is_empty() {
            return None;
        }
        self.models
            .iter()
            .filter(|model| {
                languages
                    .iter()
                    .all(|language| model.languages.contains(language))
            })
            .min_by_key(|model| model.size_bytes())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchModelInfo {
    pub id: String,
    pub title: String,
    pub languages: Vec<String>,
    pub size_bytes: u64,
    pub available: bool,
}

/// Embeddings must be finite unit vectors before the host persists or ranks them.
pub fn validate_embeddings(vectors: &[Vec<f32>], dimensions: usize) -> bool {
    !vectors.is_empty()
        && vectors.len() <= 128
        && vectors.iter().all(|vector| {
            vector.len() == dimensions
                && vector.iter().all(|value| value.is_finite())
                && (vector
                    .iter()
                    .map(|value| f64::from(*value).powi(2))
                    .sum::<f64>()
                    - 1.0)
                    .abs()
                    < 0.01
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, languages: &[&str], bytes: u64) -> SearchModel {
        SearchModel {
            id: id.into(),
            title: id.into(),
            languages: languages
                .iter()
                .map(|language| (*language).into())
                .collect(),
            dimensions: 384,
            max_tokens: 256,
            query_prefix: String::new(),
            passage_prefix: String::new(),
            minimum_similarity: 0.3,
            files: ["model.onnx", "tokenizer.json"]
                .into_iter()
                .map(|path| SearchModelFile {
                    path: path.into(),
                    url: format!(
                        "https://huggingface.co/test/model/resolve/{}/{path}",
                        "a".repeat(40)
                    ),
                    sha256: "b".repeat(64),
                    size_bytes: bytes,
                })
                .collect(),
        }
    }

    #[test]
    fn chooses_one_smallest_profile_covering_all_selected_languages() {
        let provider = ModuleSearchProvider {
            command: "embed-text".into(),
            language_field: "languages".into(),
            models: vec![
                model("english", &["en"], 10),
                model("multilingual", &["en", "uk", "de"], 50),
            ],
        };
        assert_eq!(provider.model_for(&["en".into()]).unwrap().id, "english");
        assert_eq!(
            provider.model_for(&["en".into(), "uk".into()]).unwrap().id,
            "multilingual"
        );
        assert!(provider.model_for(&[]).is_none());
        assert!(provider.model_for(&["unknown".into()]).is_none());
    }

    #[test]
    fn model_identity_reuses_identical_assets_across_language_and_threshold_changes() {
        let original = model("english", &["en"], 10);
        let mut edited = original.clone();
        edited.minimum_similarity = 0.5;
        edited.languages.push("uk".into());
        assert_eq!(
            original.resource_directory_name(),
            edited.resource_directory_name()
        );
        edited.files[0].sha256 = "c".repeat(64);
        assert_ne!(
            original.resource_directory_name(),
            edited.resource_directory_name()
        );
    }

    #[test]
    fn rejects_unpinned_resources_and_non_unit_vectors() {
        let mut profile = model("english", &["en"], 10);
        profile.validate().unwrap();
        profile.files[0].url = "https://huggingface.co/test/model/resolve/main/model.onnx".into();
        assert!(profile.validate().is_err());
        assert!(!validate_embeddings(&[vec![f32::NAN, 0.0]], 2));
        assert!(!validate_embeddings(&[vec![2.0, 0.0]], 2));
        assert!(validate_embeddings(&[vec![1.0, 0.0]], 2));
    }
}
