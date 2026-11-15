//! Explicit native model evidence on staged package and model assets.
use copypaste_module_sdk::{Module, ModuleEnvironment, ModuleInvocation, ModuleOutput};
use copypaste_module_semantic_search::SemanticSearchModule;
use std::{collections::BTreeMap, path::PathBuf, time::Instant};

fn vector(
    module: &mut SemanticSearchModule,
    languages: &[&str],
    role: &str,
    text: &str,
) -> Vec<f32> {
    let result = module
        .invoke(ModuleInvocation {
            command: "embed-text".into(),
            arguments: BTreeMap::from([("text".into(), text.into()), ("role".into(), role.into())]),
            preferences: BTreeMap::from([(
                "languages".into(),
                copypaste_module_sdk::serde_json::json!(languages),
            )]),
        })
        .expect("native embedding");
    let ModuleOutput::Embeddings { vectors, .. } = result else {
        panic!("embedding result");
    };
    assert_eq!(vectors.len(), 1);
    vectors.into_iter().next().unwrap()
}

fn similarity(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    assert_eq!(
        arguments.len(),
        4,
        "usage: semantic-evidence <package-dir> <data-dir> <receipt>"
    );
    let mut module = SemanticSearchModule::create(ModuleEnvironment {
        package_dir: arguments[1].clone(),
        data_dir: arguments[2].clone(),
    })
    .unwrap();
    let mut cases = Vec::new();
    for (languages, query, relevant, unrelated) in [
        (
            vec!["en"],
            "paying for housing",
            "Apartment lease rent for October",
            "Chocolate cake recipe with walnuts",
        ),
        (
            vec!["uk", "en"],
            "оплата житла",
            "Оренда квартири за жовтень",
            "Рецепт шоколадного торта з горіхами",
        ),
        (
            vec!["uk", "en"],
            "оплата житла",
            "Apartment lease rent for October",
            "Chocolate cake recipe with walnuts",
        ),
    ] {
        let started = Instant::now();
        let query_vector = vector(&mut module, &languages, "query", query);
        let relevant_vector = vector(&mut module, &languages, "passage", relevant);
        let unrelated_vector = vector(&mut module, &languages, "passage", unrelated);
        let relevant_score = similarity(&query_vector, &relevant_vector);
        let unrelated_score = similarity(&query_vector, &unrelated_vector);
        println!(
            "{:?}: relevant={relevant_score:.4}, unrelated={unrelated_score:.4}, elapsed={}ms",
            languages,
            started.elapsed().as_millis()
        );
        let minimum = if languages == ["en"] { 0.30 } else { 0.80 };
        assert!(
            relevant_score >= minimum,
            "the relevant clip must be retrieved"
        );
        assert!(
            unrelated_score < minimum,
            "the unrelated clip must be excluded"
        );
        cases.push(copypaste_module_sdk::serde_json::json!({"languages":languages,"relevant_similarity":relevant_score,"unrelated_similarity":unrelated_score,"elapsed_ms":started.elapsed().as_millis()}));
    }
    let receipt = copypaste_module_sdk::serde_json::json!({"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"cases":cases});
    std::fs::write(
        PathBuf::from(&arguments[3]),
        copypaste_module_sdk::serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}
