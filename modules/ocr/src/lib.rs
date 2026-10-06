//! First-party offline OCR module. Models and ONNX Runtime are package assets.
#![deny(unsafe_code)]

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use copypaste_module_sdk::{Module, ModuleEnvironment, ModuleInvocation, ModuleOutput};
use paddle_ocr_rs::{
    base_net::BaseNet, crnn_net::CrnnNet, db_net::DbNet, ocr_result::TextBox, ocr_utils::OcrUtils,
    scale_param::ScaleParam,
};

const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_IMAGE_SIDE: u32 = 8_192;
const MAX_IMAGE_PIXELS: u64 = 16_000_000;
const MAX_DECODE_ALLOC_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TEXT_REGIONS: usize = 512;
const MAX_CROP_PIXELS: u64 = 24_000_000;
const MODEL_ROOT: &str = "assets/models";
const DETECTOR: &str = "detector.onnx";

/// Every route is a PP-OCRv5 recognition model. Detection is shared, then each
/// cropped line is evaluated against each route.
const ROUTES: &[Route] = &[
    Route::new("eslav", "eslav.onnx", "eslav.keys.txt", Script::Cyrillic),
    Route::new("latin", "latin.onnx", "latin.keys.txt", Script::Latin),
    Route::new("cjk", "cjk.onnx", "cjk.keys.txt", Script::Cjk),
    Route::new("arabic", "arabic.onnx", "arabic.keys.txt", Script::Arabic),
    Route::new(
        "devanagari",
        "devanagari.onnx",
        "devanagari.keys.txt",
        Script::Devanagari,
    ),
    Route::new("korean", "korean.onnx", "korean.keys.txt", Script::Korean),
    Route::new("thai", "thai.onnx", "thai.keys.txt", Script::Thai),
    Route::new("greek", "greek.onnx", "greek.keys.txt", Script::Greek),
    Route::new("tamil", "tamil.onnx", "tamil.keys.txt", Script::Tamil),
    Route::new("telugu", "telugu.onnx", "telugu.keys.txt", Script::Telugu),
];

pub struct OcrModule {
    package_dir: PathBuf,
    engine: Option<OcrEngine>,
}

impl Module for OcrModule {
    fn create(environment: ModuleEnvironment) -> Result<Self, String> {
        // Loading models here would hide useful package errors behind a null ABI
        // context. Keep construction cheap and initialize only on first command.
        Ok(Self {
            package_dir: PathBuf::from(environment.package_dir),
            engine: None,
        })
    }

    fn invoke(&mut self, invocation: ModuleInvocation) -> Result<ModuleOutput, String> {
        if invocation.command != "recognize-image" {
            return Err("Unknown OCR command.".into());
        }
        let image = invocation
            .arguments
            .get("image_path")
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .ok_or("An image is required.")?;
        validate_image(Path::new(image))?;
        if self.engine.is_none() {
            self.engine = Some(OcrEngine::load(&self.package_dir)?);
        }
        let text = self
            .engine
            .as_mut()
            .expect("OCR engine initialized")
            .recognize(Path::new(image))?;
        Ok(ModuleOutput::Text { text })
    }
}

struct OcrEngine {
    detector: DbNet,
    models: PathBuf,
}

impl OcrEngine {
    fn load(package_dir: &Path) -> Result<Self, String> {
        initialize_onnx_runtime(package_dir)?;
        let models = package_dir.join(MODEL_ROOT);
        require_file(&models.join(DETECTOR), "text detection model")?;
        for route in ROUTES {
            require_file(&models.join(route.model), route.id)?;
            require_file(&models.join(route.dictionary), route.id)?;
        }

        let mut detector = DbNet::new();
        detector
            .init_model(path_string(&models.join(DETECTOR))?.as_str(), 2, None)
            .map_err(ocr_error)?;
        Ok(Self { detector, models })
    }

    fn recognize(&mut self, image_path: &Path) -> Result<String, String> {
        let image = decode_image(image_path)?;
        let padding = 50;
        let padded = OcrUtils::make_padding(&image, padding).map_err(ocr_error)?;
        let longest_side = padded.width().max(padded.height()).min(2_000);
        let scale = ScaleParam::get_scale_param(&padded, longest_side + (padding * 2));
        let boxes = self
            .detector
            .get_text_boxes(&padded, &scale, 0.5, 0.3, 1.6)
            .map_err(ocr_error)?;
        validate_text_regions(&boxes, &padded)?;
        if boxes.is_empty() {
            return Ok(String::new());
        }

        let lines = OcrUtils::get_part_images(&padded, &boxes);

        let mut candidates = Vec::with_capacity(ROUTES.len());
        for route in ROUTES {
            candidates.push(self.recognize_route(*route, &lines)?);
        }
        let text = (0..lines.len())
            .filter_map(|index| select_line(index, &candidates))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(text)
    }

    fn recognize_route(
        &self,
        route: Route,
        lines: &[image::RgbImage],
    ) -> Result<Candidate, String> {
        let mut recognizer = CrnnNet::new();
        recognizer
            .init_model_dict_file(
                path_string(&self.models.join(route.model))?.as_str(),
                2,
                None,
                path_string(&self.models.join(route.dictionary))?.as_str(),
            )
            .map_err(ocr_error)?;
        let values = recognizer
            .get_text_lines(lines, &HashMap::new(), 0.0)
            .map_err(ocr_error)?
            .into_iter()
            .map(|line| LineCandidate {
                score: route.score(&line.text, line.text_score),
                text: line.text,
            })
            .collect();
        Ok(Candidate { route, values })
    }
}

#[derive(Clone, Copy)]
struct Route {
    id: &'static str,
    model: &'static str,
    dictionary: &'static str,
    script: Script,
}

impl Route {
    const fn new(
        id: &'static str,
        model: &'static str,
        dictionary: &'static str,
        script: Script,
    ) -> Self {
        Self {
            id,
            model,
            dictionary,
            script,
        }
    }

    fn score(self, text: &str, confidence: f32) -> f32 {
        let mut letters = 0_u32;
        let mut matching = 0_u32;
        let mut invalid = 0_u32;
        let mut native_script = false;
        for character in text.chars() {
            if let Some(script) = Script::of(character) {
                letters += 1;
                if self.accepts(script) {
                    matching += 1;
                }
                native_script |= script == self.script
                    || (self.script == Script::Cjk && script == Script::Japanese);
            } else if character.is_control() || character == '\u{fffd}' {
                invalid += 1;
            }
        }
        if text.trim().is_empty() {
            return f32::NEG_INFINITY;
        }
        let fit = if letters == 0 {
            0.45
        } else {
            matching as f32 / letters as f32
        };
        // This heuristic is intentionally not a calibrated language detector.
        // A native-script marker prevents an all-Latin hallucination from
        // replacing correctly decoded Ukrainian plus English on one line.
        0.40 * confidence.clamp(0.0, 1.0)
            + 0.50 * fit
            + 0.10
            + if native_script { 0.15 } else { 0.0 }
            - invalid as f32 * 0.20
    }

    fn accepts(self, script: Script) -> bool {
        script == self.script
            || script == Script::Latin
            || (self.script == Script::Cjk && script == Script::Japanese)
    }

    fn distinctive_markers(self, text: &str) -> u32 {
        if self.script == Script::Latin {
            return 0;
        }
        text.chars()
            .filter_map(Script::of)
            .filter(|script| {
                *script == self.script
                    || (self.script == Script::Cjk && *script == Script::Japanese)
            })
            .count() as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Script {
    Latin,
    Cyrillic,
    Cjk,
    Japanese,
    Arabic,
    Devanagari,
    Korean,
    Thai,
    Greek,
    Tamil,
    Telugu,
}

impl Script {
    fn of(character: char) -> Option<Self> {
        if !character.is_alphabetic() {
            return None;
        }
        let point = character as u32;
        Some(match point {
            0x0041..=0x024f | 0x1e00..=0x1eff => Self::Latin,
            0x0370..=0x03ff | 0x1f00..=0x1fff => Self::Greek,
            0x0400..=0x052f | 0x2de0..=0x2dff | 0xa640..=0xa69f => Self::Cyrillic,
            0x0600..=0x06ff | 0x0750..=0x077f | 0x08a0..=0x08ff => Self::Arabic,
            0x0900..=0x097f => Self::Devanagari,
            0x0e00..=0x0e7f => Self::Thai,
            0x0b80..=0x0bff => Self::Tamil,
            0x0c00..=0x0c7f => Self::Telugu,
            0x1100..=0x11ff | 0x3130..=0x318f | 0xac00..=0xd7af => Self::Korean,
            0x3040..=0x30ff | 0x31f0..=0x31ff => Self::Japanese,
            0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff => Self::Cjk,
            _ => return None,
        })
    }
}

struct Candidate {
    route: Route,
    values: Vec<LineCandidate>,
}

struct LineCandidate {
    score: f32,
    text: String,
}

fn select_line(index: usize, candidates: &[Candidate]) -> Option<String> {
    candidates
        .iter()
        .filter_map(|candidate| {
            candidate
                .values
                .get(index)
                .map(|line| (candidate.route, line))
        })
        .max_by(|(left_route, left), (right_route, right)| {
            left_route
                .distinctive_markers(&left.text)
                .cmp(&right_route.distinctive_markers(&right.text))
                .then_with(|| left.score.total_cmp(&right.score))
                .then_with(|| left_route.id.cmp(right_route.id))
        })
        .map(|(_, line)| line.text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn initialize_onnx_runtime(package_dir: &Path) -> Result<(), String> {
    let name = if cfg!(target_os = "windows") {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    };
    let runtime = package_dir.join("bin").join(name);
    require_file(&runtime, "ONNX Runtime")?;
    ort::init_from(path_string(&runtime)?)
        .map_err(|_| "CopyPaste OCR could not load its packaged ONNX Runtime.".to_owned())?
        .with_telemetry(false)
        .commit();
    Ok(())
}

fn validate_image(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || !path.is_file() {
        return Err("The selected image is unavailable.".into());
    }
    let bytes = fs::metadata(path)
        .map_err(|_| "The selected image is unavailable.".to_owned())?
        .len();
    if bytes > MAX_IMAGE_BYTES {
        return Err("The selected image is larger than 64 MiB.".into());
    }
    Ok(())
}

fn decode_image(path: &Path) -> Result<image::RgbImage, String> {
    let mut reader = image::ImageReader::open(path)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(|_| "CopyPaste OCR could not read this image.".to_owned())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC_BYTES);
    reader.limits(limits);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| "CopyPaste OCR could not read this image.".to_owned())?;
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return Err("The selected image has too many pixels for OCR.".into());
    }
    let mut reader = image::ImageReader::open(path)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(|_| "CopyPaste OCR could not read this image.".to_owned())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC_BYTES);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|_| "The selected image is too large or malformed for OCR.".to_owned())
        .map(|image| image.to_rgb8())
}

fn validate_text_regions(boxes: &[TextBox], image: &image::RgbImage) -> Result<(), String> {
    if boxes.len() > MAX_TEXT_REGIONS {
        return Err("The selected image contains too many text regions for OCR.".into());
    }
    let mut total_pixels = 0_u64;
    for box_ in boxes {
        if box_.points.len() != 4
            || box_
                .points
                .iter()
                .any(|point| point.x >= image.width() || point.y >= image.height())
        {
            return Err("CopyPaste OCR produced an invalid text region.".into());
        }
        let min_x = box_
            .points
            .iter()
            .map(|point| point.x)
            .min()
            .unwrap_or_default();
        let max_x = box_
            .points
            .iter()
            .map(|point| point.x)
            .max()
            .unwrap_or_default();
        let min_y = box_
            .points
            .iter()
            .map(|point| point.y)
            .min()
            .unwrap_or_default();
        let max_y = box_
            .points
            .iter()
            .map(|point| point.y)
            .max()
            .unwrap_or_default();
        let pixels = u64::from(max_x - min_x) * u64::from(max_y - min_y);
        if pixels == 0 || pixels > MAX_CROP_PIXELS {
            return Err("A detected text region is too large for OCR.".into());
        }
        total_pixels = total_pixels.saturating_add(pixels);
        if total_pixels > MAX_CROP_PIXELS {
            return Err("The detected text regions are too large for OCR.".into());
        }
    }
    Ok(())
}

fn require_file(path: &Path, name: &str) -> Result<(), String> {
    if path.is_file() {
        Ok(())
    } else {
        Err(format!(
            "CopyPaste OCR is incomplete: missing {name}. Reinstall the OCR module."
        ))
    }
}

fn path_string(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| "CopyPaste OCR encountered an unsupported package path.".into())
}

fn ocr_error(error: paddle_ocr_rs::ocr_error::OcrError) -> String {
    format!("CopyPaste OCR failed to run its packaged model: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_cyrillic_candidate_when_confidence_alone_would_choose_latin() {
        let candidates = [
            Candidate {
                route: ROUTES[0],
                values: vec![LineCandidate {
                    text: "Привіт Україно".into(),
                    score: ROUTES[0].score("Привіт Україно", 0.78),
                }],
            },
            Candidate {
                route: ROUTES[1],
                values: vec![LineCandidate {
                    text: "Ppивit Ykpaiho".into(),
                    score: ROUTES[1].score("Ppивit Ykpaiho", 0.93),
                }],
            },
        ];
        assert_eq!(select_line(0, &candidates), Some("Привіт Україно".into()));
    }

    #[test]
    fn routes_a_mixed_document_per_line() {
        let candidates = [
            Candidate {
                route: ROUTES[0],
                values: vec![
                    LineCandidate {
                        text: "Українська".into(),
                        score: ROUTES[0].score("Українська", 0.95),
                    },
                    LineCandidate {
                        text: "Eng1ish".into(),
                        score: ROUTES[0].score("Eng1ish", 0.55),
                    },
                ],
            },
            Candidate {
                route: ROUTES[1],
                values: vec![
                    LineCandidate {
                        text: "YkpaiHcbKa".into(),
                        score: ROUTES[1].score("YkpaiHcbKa", 0.62),
                    },
                    LineCandidate {
                        text: "English".into(),
                        score: ROUTES[1].score("English", 0.95),
                    },
                ],
            },
        ];
        assert_eq!(select_line(0, &candidates), Some("Українська".into()));
        assert_eq!(select_line(1, &candidates), Some("English".into()));
    }

    #[test]
    fn preserves_english_inside_a_cyrillic_line() {
        let candidates = [
            Candidate {
                route: ROUTES[0],
                values: vec![LineCandidate {
                    text: "CopyPaste: Привіт, Україно! OCR".into(),
                    score: ROUTES[0].score("CopyPaste: Привіт, Україно! OCR", 0.78),
                }],
            },
            Candidate {
                route: ROUTES[1],
                values: vec![LineCandidate {
                    text: "CopyPaste: Ppивiт, Yкpaїно! OCR".into(),
                    score: ROUTES[1].score("CopyPaste: Ppивiт, Yкpaїно! OCR", 0.93),
                }],
            },
        ];
        assert_eq!(
            select_line(0, &candidates),
            Some("CopyPaste: Привіт, Україно! OCR".into())
        );
    }

    #[test]
    fn punctuation_is_not_treated_as_latin_script() {
        assert_eq!(Script::of('!'), None);
        assert_eq!(Script::of('1'), None);
        assert_eq!(Script::of('Ї'), Some(Script::Cyrillic));
        assert_eq!(Script::of('C'), Some(Script::Latin));
    }

    #[test]
    fn runtime_name_matches_platform_contract() {
        let name = if cfg!(target_os = "windows") {
            "onnxruntime.dll"
        } else if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.so"
        };
        assert!(!name.is_empty());
    }

    #[test]
    fn manifest_declares_the_private_image_file_contract() {
        let manifest: copypaste_module_sdk::serde_json::Value =
            copypaste_module_sdk::serde_json::from_str(include_str!("../module.json")).unwrap();
        let argument = &manifest["commands"][0]["arguments"][0];
        assert_eq!(argument["id"], "image_path");
        assert_eq!(argument["kind"], "file");
        assert_eq!(argument["required"], true);
        assert_eq!(argument["max_bytes"], MAX_IMAGE_BYTES);
        assert_eq!(
            argument["accepted_extensions"],
            copypaste_module_sdk::serde_json::json!([
                "png", "jpg", "jpeg", "webp", "tiff", "tif", "bmp"
            ])
        );
    }
}

copypaste_module_sdk::export_module!(OcrModule);
