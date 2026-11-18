//! Loads `tokenizer.json`, keeping Unigram vocabularies in compact storage.
use crate::unigram::CompactUnigram;
use copypaste_module_sdk::serde_json;
use serde::Deserialize;
use std::{borrow::Cow, fs, path::Path};
use tokenizers::{
    DecoderWrapper, Encoding, NormalizerWrapper, PostProcessorWrapper, PreTokenizerWrapper, Result,
    Tokenizer, TokenizerImpl, TruncationParams,
};

type CompactTokenizer = TokenizerImpl<
    CompactUnigram,
    NormalizerWrapper,
    PreTokenizerWrapper,
    PostProcessorWrapper,
    DecoderWrapper,
>;

pub enum TextTokenizer {
    Compact(Box<CompactTokenizer>),
    Library(Box<Tokenizer>),
}

#[derive(Deserialize)]
struct Document<'a> {
    #[serde(borrow)]
    model: ModelKind<'a>,
}

#[derive(Deserialize)]
struct ModelKind<'a> {
    #[serde(rename = "type", borrow)]
    kind: Option<Cow<'a, str>>,
}

impl TextTokenizer {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let unigram = serde_json::from_str::<Document>(&content)
            .is_ok_and(|document| document.model.kind.as_deref() == Some("Unigram"));
        Ok(if unigram {
            Self::Compact(serde_json::from_str(&content)?)
        } else {
            Self::Library(serde_json::from_str(&content)?)
        })
    }

    pub fn with_truncation(&mut self, truncation: Option<TruncationParams>) -> Result<()> {
        match self {
            Self::Compact(tokenizer) => tokenizer.with_truncation(truncation).map(|_| ()),
            Self::Library(tokenizer) => tokenizer.with_truncation(truncation).map(|_| ()),
        }
    }

    pub fn without_padding(&mut self) {
        match self {
            Self::Compact(tokenizer) => {
                tokenizer.with_padding(None);
            }
            Self::Library(tokenizer) => {
                tokenizer.with_padding(None);
            }
        }
    }

    pub fn encode(&self, text: &str, add_special_tokens: bool) -> Result<Encoding> {
        match self {
            Self::Compact(tokenizer) => tokenizer.encode(text, add_special_tokens),
            Self::Library(tokenizer) => tokenizer.encode(text, add_special_tokens),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_module_sdk::serde_json::json;
    use tokenizers::Model;

    struct Random(u64);

    impl Random {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: usize) -> usize {
            (self.next() % bound as u64) as usize
        }

        fn pick<'a, T>(&mut self, values: &'a [T]) -> &'a T {
            &values[self.below(values.len())]
        }
    }

    fn load_both(path: &Path) -> (CompactTokenizer, Tokenizer) {
        let TextTokenizer::Compact(compact) = TextTokenizer::from_file(path).unwrap() else {
            panic!("a Unigram vocabulary must load into the compact model");
        };
        (*compact, Tokenizer::from_file(path).unwrap())
    }

    fn truncate(
        compact: &mut CompactTokenizer,
        library: &mut Tokenizer,
        max_length: usize,
        stride: usize,
    ) {
        let truncation = Some(TruncationParams {
            max_length,
            stride,
            ..Default::default()
        });
        compact.with_truncation(truncation.clone()).unwrap();
        library.with_truncation(truncation).unwrap();
    }

    /// Asserts identical encodings, overflow included, and returns the token count.
    fn compare(compact: &CompactTokenizer, library: &Tokenizer, text: &str) -> usize {
        let mut tokens = 0;
        for special in [true, false] {
            let expected = library.encode(text, special).unwrap();
            assert_eq!(compact.encode(text, special).unwrap(), expected, "{text:?}");
            tokens += expected.len()
                + expected
                    .get_overflowing()
                    .iter()
                    .map(Encoding::len)
                    .sum::<usize>();
        }
        tokens
    }

    fn assert_same_vocabulary(compact: &CompactTokenizer, library: &Tokenizer) {
        let model = compact.get_model();
        let reference = library.get_model();
        assert_eq!(model.get_vocab_size(), reference.get_vocab_size());
        assert_eq!(model.get_vocab(), reference.get_vocab());
        for id in 0..=model.get_vocab_size() as u32 {
            let token = model.id_to_token(id);
            assert_eq!(token, reference.id_to_token(id));
            if let Some(token) = token {
                assert_eq!(model.token_to_id(&token), reference.token_to_id(&token));
            }
        }
        assert_eq!(compact.get_vocab(true), library.get_vocab(true));
    }

    #[test]
    fn compact_unigram_matches_library_encodings() {
        let mut random = Random(0x9e37_79b9_7f4a_7c15);
        let alphabet: Vec<char> = "abcdefghijklmnoprstuvwxyzабвгдеиклмнопрстуяі中文字日本語"
            .chars()
            .collect();
        let mut vocab = vec![
            json!(["<s>", 0.0]),
            json!(["<pad>", 0.0]),
            json!(["</s>", 0.0]),
            json!(["<unk>", 0.0]),
            json!(["▁", -2.0]),
        ];
        for letter in &alphabet {
            vocab.push(json!([
                letter.to_string(),
                -8.0 - random.below(40) as f64 / 10.0
            ]));
        }
        for byte in 0x20..0x7f_u8 {
            vocab.push(json!([format!("<0x{byte:02X}>"), -30.0]));
        }
        // `ж`, `q` and `é` have no single-character piece; `жи` and `qz` exist.
        for piece in [
            "жи",
            "qz",
            "▁жи",
            "▁прив",
            "▁при",
            "▁пр",
            "ab",
            "abc",
            "abcd",
            "中文",
        ] {
            vocab.push(json!([piece, -3.0 - random.below(30) as f64 / 10.0]));
        }
        for _ in 0..400 {
            let mut piece = String::new();
            if random.below(2) == 0 {
                piece.push('▁');
            }
            for _ in 0..1 + random.below(5) {
                piece.push(*random.pick(&alphabet));
            }
            vocab.push(json!([piece, -4.0 - random.below(80) as f64 / 10.0]));
        }
        vocab.push(json!(["▁abc", -1.5]));
        vocab.push(json!(["▁abc", -9.5]));
        vocab.push(json!(["<mask>", 0.0]));
        let mask_id = vocab.len() - 1;
        let special = |id: usize, content: &str| {
            json!({"id": id, "content": content, "single_word": false, "lstrip": false,
                "rstrip": false, "normalized": false, "special": true})
        };
        let document = json!({
            "version": "1.0",
            "truncation": null,
            "padding": null,
            "added_tokens": [special(0, "<s>"), special(1, "<pad>"), special(2, "</s>"),
                special(3, "<unk>"), special(mask_id, "<mask>")],
            "normalizer": {"type": "Sequence", "normalizers": [{"type": "NFKC"},
                {"type": "Replace", "pattern": {"Regex": " {2,}"}, "content": " "}]},
            "pre_tokenizer": {"type": "Metaspace", "replacement": "▁", "add_prefix_space": true},
            "post_processor": {"type": "TemplateProcessing",
                "single": [{"SpecialToken": {"id": "<s>", "type_id": 0}},
                    {"Sequence": {"id": "A", "type_id": 0}},
                    {"SpecialToken": {"id": "</s>", "type_id": 0}}],
                "pair": [{"Sequence": {"id": "A", "type_id": 0}},
                    {"Sequence": {"id": "B", "type_id": 1}}],
                "special_tokens": {"<s>": {"id": "<s>", "ids": [0], "tokens": ["<s>"]},
                    "</s>": {"id": "</s>", "ids": [2], "tokens": ["</s>"]}}},
            "decoder": {"type": "Metaspace", "replacement": "▁", "add_prefix_space": true},
            "model": {"type": "Unigram", "unk_id": 3, "byte_fallback": true, "vocab": vocab},
        });
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tokenizer.json");
        fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        let (mut compact, mut library) = load_both(&path);
        assert_same_vocabulary(&compact, &library);
        let fused = library.encode("жжé q", false).unwrap();
        assert_eq!(fused.get_tokens(), ["▁", "жжé", "▁", "<0x71>"]);
        assert_eq!(fused.get_ids()[1], 3);
        truncate(&mut compact, &mut library, 16, 4);

        let mut pool = alphabet.clone();
        pool.extend("жqéé  \t\n😀👍🏽\u{301}<>".chars());
        let fixed = [
            "",
            " ",
            "abcd abc ab",
            "жи qz qzq жжи",
            "привіт <mask> </s>",
            "中文字中",
        ];
        let mut tokens = 0;
        for text in fixed {
            tokens += compare(&compact, &library, text);
        }
        for _ in 0..3_000 {
            let text: String = (0..random.below(60)).map(|_| *random.pick(&pool)).collect();
            tokens += compare(&compact, &library, &text);
        }
        assert!(tokens > 50_000, "{tokens}");
    }

    /// Opt-in: `SEMANTIC_SEARCH_TOKENIZER=<path to tokenizer.json>`.
    #[test]
    fn compact_unigram_matches_library_on_installed_vocabulary() {
        let Some(path) = std::env::var_os("SEMANTIC_SEARCH_TOKENIZER") else {
            return;
        };
        let (mut compact, mut library) = load_both(Path::new(&path));
        assert_same_vocabulary(&compact, &library);
        let prefix = library.encode("passage: ", false).unwrap().len();
        truncate(&mut compact, &mut library, 512 - prefix, 64);

        let samples = [
            "Invoice 4821 for October rent was paid on Friday. Café, naïve, Ærøskøbing, Łódź.",
            "Оренда квартири за жовтень сплачена з рахунку в п'ятницю ввечері. Ёлка, їжак, ґанок.",
            "混合中文文本，包括标点符号。日本語のテキストとカタカナ、ひらがな。한국어 문장입니다.",
            "مرحبا بالعالم، هذا نص عربي مع أرقام ١٢٣ وتشكيل: كَتَبَ الوَلَدُ.",
            "यह हिंदी में एक वाक्य है। संयुक्ताक्षर: क्ष त्र ज्ञ श्र और अंक १२३।",
            "Emoji 😀👍🏽👨‍👩‍👧‍👦🏳️‍🌈 and combining marks e\u{301} a\u{308}\u{323} Z\u{335}\u{34f}",
            "  leading,   multiple    spaces\t\ttabs\n\nnewlines \u{a0}\u{2003}\u{3000} wide  ",
            "<s> literal special </s> <unk> <mask> <pad> ▁▁ ▁ ________ ...... ---- ****",
            "https://example.com/path?query=1&x=2 user@example.org 0x1F 3.14159 1e-10",
        ];
        let mut inputs = 0;
        let mut tokens = 0;
        let mut check = |text: &str| {
            inputs += 1;
            tokens += compare(&compact, &library, text);
        };
        for sample in samples {
            check(sample);
            for length in [1, 7, 40] {
                check(&sample.chars().take(length).collect::<String>());
            }
        }
        for (index, length) in [300, 2_500, 6_000, 24_000].into_iter().enumerate() {
            let text: String = samples
                .iter()
                .cycle()
                .skip(index)
                .flat_map(|sample| sample.chars().chain([' ']))
                .take(length)
                .collect();
            check(&text);
        }
        let ranges = [
            (0x20, 0x7f),
            (0xa0, 0x250),
            (0x300, 0x370),
            (0x370, 0x530),
            (0x590, 0x700),
            (0x900, 0xe00),
            (0x1100, 0x1200),
            (0x2000, 0x2c00),
            (0x3000, 0x3100),
            (0x4e00, 0x9fff),
            (0xac00, 0xd7a4),
            (0xe000, 0xe100),
            (0xfb00, 0xfffe),
            (0x1f300, 0x1fb00),
            (0x20000, 0x20100),
            (0x0, 0x110000),
        ];
        let mut random = Random(0x2545_f491_4f6c_dd1d);
        for round in 0..6_000 {
            let length = if round % 500 == 0 {
                24_000
            } else {
                random.below(200)
            };
            let text: String = (0..length)
                .filter_map(|_| {
                    let (low, high) = *random.pick(&ranges);
                    char::from_u32((low + random.below(high - low)) as u32)
                })
                .collect();
            check(&text);
        }
        println!("compared {inputs} inputs and {tokens} tokens");
    }
}
