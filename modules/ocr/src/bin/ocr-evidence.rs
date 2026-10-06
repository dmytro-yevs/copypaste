//! Runs model evidence in its own process because ONNX Runtime owns globals.

use copypaste_module_ocr::OcrModule;
use copypaste_module_sdk::{Module, ModuleEnvironment, ModuleInvocation, ModuleOutput};
use std::collections::BTreeMap;

fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

fn main() {
    let package_dir = required("COPYPASTE_OCR_EVIDENCE_PACKAGE");
    let image_path = required("COPYPASTE_OCR_EVIDENCE_IMAGE");
    let expected = required("COPYPASTE_OCR_EVIDENCE_TEXT");
    let result = (|| -> Result<String, String> {
        let mut module = OcrModule::create(ModuleEnvironment {
            package_dir,
            data_dir: String::new(),
        })?;
        let output = module.invoke(ModuleInvocation {
            command: "recognize-image".into(),
            arguments: BTreeMap::from([(
                "image_path".into(),
                copypaste_module_sdk::serde_json::Value::String(image_path),
            )]),
            preferences: BTreeMap::new(),
        })?;
        let ModuleOutput::Text { text } = output else {
            return Err("OCR command did not return text.".into());
        };
        if !text.contains(&expected) {
            return Err(format!("OCR output did not contain {expected:?}: {text:?}"));
        }
        Ok(text)
    })();
    match result {
        Ok(text) => {
            println!("{text}");
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
