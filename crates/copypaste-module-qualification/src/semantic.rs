//! Real offline inference through the production package boundary.
use copypaste_module_sdk::ModuleSearchProvider;
use copypaste_modules::{ModuleManager, ModuleOutput};
use serde_json::json;
use std::{collections::BTreeMap, fs, path::Path};

pub(super) fn qualify(
    manager: &ModuleManager,
    fixtures: &Path,
    data: &Path,
    id: &str,
) -> Result<usize, String> {
    let provider: ModuleSearchProvider = serde_json::from_slice(
        &fs::read(fixtures.join("search-models.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    for model in &provider.models {
        model.validate()?;
        let destination = data
            .join("data")
            .join(id)
            .join("models")
            .join(model.resource_directory_name());
        fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
        for file in &model.files {
            let path = destination.join(&file.path);
            fs::create_dir_all(path.parent().ok_or("Invalid model path")?)
                .map_err(|e| e.to_string())?;
            fs::copy(fixtures.join(&model.id).join(&file.path), path).map_err(|e| e.to_string())?;
        }
    }
    for (languages, query, relevant, unrelated, minimum) in [
        (
            vec!["en"],
            "paying for housing",
            "Apartment lease rent for October",
            "Chocolate cake recipe with walnuts",
            0.30,
        ),
        (
            vec!["uk", "en"],
            "оплата житла",
            "Оренда квартири за жовтень",
            "Рецепт шоколадного торта з горіхами",
            0.80,
        ),
        (
            vec!["uk", "en"],
            "оплата житла",
            "Apartment lease rent for October",
            "Chocolate cake recipe with walnuts",
            0.80,
        ),
    ] {
        manager
            .set_preferences(id, BTreeMap::from([("languages".into(), json!(languages))]))
            .map_err(|e| e.to_string())?;
        manager.set_enabled(id, true).map_err(|e| e.to_string())?;
        let query = vector(manager, id, "query", query)?;
        let relevant = vector(manager, id, "passage", relevant)?;
        let unrelated = vector(manager, id, "passage", unrelated)?;
        let similarity = |other: &[f32]| query.iter().zip(other).map(|(a, b)| a * b).sum::<f32>();
        if similarity(&relevant) < minimum || similarity(&unrelated) >= minimum {
            return Err(format!(
                "Semantic retrieval failed for {languages:?}: relevant={}, unrelated={}",
                similarity(&relevant),
                similarity(&unrelated)
            ));
        }
    }
    manager.set_enabled(id, false).map_err(|e| e.to_string())?;
    if vector(manager, id, "query", "disabled").is_ok() {
        return Err("Disabled semantic module accepted inference.".into());
    }
    manager.set_enabled(id, true).map_err(|e| e.to_string())?;
    vector(manager, id, "query", "оплата житла")?;
    Ok(3)
}

fn vector(manager: &ModuleManager, id: &str, role: &str, text: &str) -> Result<Vec<f32>, String> {
    let output = manager
        .invoke(
            id,
            "embed-text",
            BTreeMap::from([("text".into(), json!(text)), ("role".into(), json!(role))]),
        )
        .map_err(|e| e.to_string())?;
    let ModuleOutput::Embeddings { vectors, .. } = output else {
        return Err("Semantic package did not return embeddings.".into());
    };
    if vectors.len() != 1 || vectors[0].len() != 384 || vectors[0].iter().any(|v| !v.is_finite()) {
        return Err("Semantic package returned invalid vectors.".into());
    }
    Ok(vectors.into_iter().next().ok_or("Missing embedding")?)
}
