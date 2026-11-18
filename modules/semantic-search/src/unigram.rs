//! Unigram vocabulary model with the library's segmentation in compact storage.
use serde::de::{
    self, DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Unexpected, Visitor,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
};
use tokenizers::{models::unigram::UnigramTrainer, Model, Result, Token};

const UNKNOWN_PENALTY: f64 = 10.0;
const NO_PIECE: u32 = u32::MAX;

/// Pieces are stored once; the trie keeps children contiguous and sorted by
/// byte, with the piece id at the node that ends it.
pub struct CompactUnigram {
    pieces: Box<str>,
    piece_ends: Box<[u32]>,
    scores: Box<[f64]>,
    trie: PieceTrie,
    min_score: f64,
    unk_id: Option<usize>,
    byte_fallback: bool,
}

struct PieceTrie {
    labels: Box<[u8]>,
    child_starts: Box<[u32]>,
    ids: Box<[u32]>,
}

#[derive(Default)]
struct Pieces {
    text: String,
    ends: Vec<u32>,
    scores: Vec<f64>,
}

impl CompactUnigram {
    fn new(pieces: Pieces, unk_id: Option<usize>, byte_fallback: bool) -> Result<Self> {
        let count = pieces.ends.len();
        if let Some(unk_id) = unk_id {
            if count == 0 {
                return Err("The vocabulary is empty but at least <unk> is needed".into());
            }
            if unk_id >= count {
                return Err("The `unk_id` is larger than vocabulary size".into());
            }
        }
        let mut min_score = f64::INFINITY;
        for score in &pieces.scores {
            if *score < min_score {
                min_score = *score;
            }
        }
        let trie = PieceTrie::build(&pieces.text, &pieces.ends);
        Ok(Self {
            pieces: pieces.text.into_boxed_str(),
            piece_ends: pieces.ends.into_boxed_slice(),
            scores: pieces.scores.into_boxed_slice(),
            trie,
            min_score,
            unk_id,
            byte_fallback,
        })
    }

    fn piece(&self, id: usize) -> Option<&str> {
        let end = *self.piece_ends.get(id)? as usize;
        let start = id
            .checked_sub(1)
            .map_or(0, |id| self.piece_ends[id] as usize);
        Some(&self.pieces[start..end])
    }

    fn missing_unknown() -> Box<dyn std::error::Error + Send + Sync> {
        "Encountered an unknown token but `unk_id` is missing".into()
    }

    /// Best segmentation as `(start, end, id)`; `None` marks fused unknown text.
    fn segment(&self, sentence: &str) -> Result<Vec<(usize, usize, Option<u32>)>> {
        #[derive(Clone)]
        struct BestPath {
            id: usize,
            score: f64,
            starts_at: Option<usize>,
        }
        let size = sentence.len();
        let bytes = sentence.as_bytes();
        let unk_score = self.min_score - UNKNOWN_PENALTY;
        let mut best = vec![
            BestPath {
                id: 0,
                score: 0.0,
                starts_at: None,
            };
            size + 1
        ];
        let mut starts_at = 0;
        while starts_at < size {
            let score_here = best[starts_at].score;
            let char_length = sentence[starts_at..]
                .chars()
                .next()
                .map_or(1, char::len_utf8);
            let mut has_single_node = false;
            let mut node = 0;
            for (offset, byte) in bytes[starts_at..].iter().enumerate() {
                let Some(child) = self.trie.child(node, *byte) else {
                    break;
                };
                node = child;
                let id = self.trie.ids[node];
                if id == NO_PIECE {
                    continue;
                }
                let length = offset + 1;
                let candidate = self.scores[id as usize] + score_here;
                let target = &mut best[starts_at + length];
                if target.starts_at.is_none() || candidate > target.score {
                    target.score = candidate;
                    target.starts_at = Some(starts_at);
                    target.id = id as usize;
                }
                if length == char_length {
                    has_single_node = true;
                }
            }
            if !has_single_node {
                let candidate = unk_score + score_here;
                let target = &mut best[starts_at + char_length];
                if target.starts_at.is_none() || candidate > target.score {
                    target.score = candidate;
                    target.starts_at = Some(starts_at);
                    target.id = self.unk_id.ok_or_else(Self::missing_unknown)?;
                }
            }
            starts_at += char_length;
        }
        let mut segments = Vec::new();
        let mut unknown_start = None;
        let mut ends_at = size;
        while ends_at > 0 {
            let node = &best[ends_at];
            let starts_at = node.starts_at.ok_or("The segmentation is incomplete")?;
            if Some(node.id) == self.unk_id {
                if unknown_start.is_none() {
                    unknown_start = Some(ends_at);
                }
            } else {
                if let Some(end) = unknown_start.take() {
                    segments.push((ends_at, end, None));
                }
                segments.push((starts_at, ends_at, Some(node.id as u32)));
            }
            ends_at = starts_at;
        }
        if let Some(end) = unknown_start {
            segments.push((0, end, None));
        }
        segments.reverse();
        Ok(segments)
    }
}

impl PieceTrie {
    fn build(text: &str, ends: &[u32]) -> Self {
        let piece = |id: u32| {
            let id = id as usize;
            let start = id.checked_sub(1).map_or(0, |id| ends[id] as usize);
            &text.as_bytes()[start..ends[id] as usize]
        };
        let mut order: Vec<u32> = (0..ends.len() as u32).collect();
        order.sort_unstable_by(|left, right| piece(*left).cmp(piece(*right)).then(left.cmp(right)));
        // The library maps a repeated piece to its last id.
        order.dedup_by(|later, earlier| {
            let same = piece(*later) == piece(*earlier);
            if same {
                *earlier = *later;
            }
            same
        });
        let mut labels = vec![0];
        let mut ids = vec![NO_PIECE];
        let mut child_starts = Vec::new();
        let mut level = vec![(0, order.len())];
        let mut depth = 0;
        let mut node = 0;
        while !level.is_empty() {
            let mut next = Vec::new();
            for (mut low, high) in level {
                if low < high && piece(order[low]).len() == depth {
                    ids[node] = order[low];
                    low += 1;
                }
                child_starts.push(labels.len() as u32);
                while low < high {
                    let label = piece(order[low])[depth];
                    let mut end = low + 1;
                    while end < high && piece(order[end])[depth] == label {
                        end += 1;
                    }
                    labels.push(label);
                    ids.push(NO_PIECE);
                    next.push((low, end));
                    low = end;
                }
                node += 1;
            }
            level = next;
            depth += 1;
        }
        child_starts.push(labels.len() as u32);
        Self {
            labels: labels.into_boxed_slice(),
            child_starts: child_starts.into_boxed_slice(),
            ids: ids.into_boxed_slice(),
        }
    }

    fn child(&self, node: usize, label: u8) -> Option<usize> {
        let start = self.child_starts[node] as usize;
        let end = self.child_starts[node + 1] as usize;
        self.labels[start..end]
            .binary_search(&label)
            .ok()
            .map(|index| start + index)
    }

    fn get(&self, key: &[u8]) -> Option<u32> {
        let mut node = 0;
        for byte in key {
            node = self.child(node, *byte)?;
        }
        Some(self.ids[node]).filter(|id| *id != NO_PIECE)
    }
}

impl Model for CompactUnigram {
    type Trainer = UnigramTrainer;

    fn tokenize(&self, sentence: &str) -> Result<Vec<Token>> {
        if sentence.is_empty() {
            return Ok(Vec::new());
        }
        let segments = self.segment(sentence)?;
        let mut tokens = Vec::with_capacity(segments.len());
        for (start, end, id) in segments {
            let text = &sentence[start..end];
            let id = match id.or_else(|| self.trie.get(text.as_bytes())) {
                Some(id) => id,
                None => {
                    if self.byte_fallback {
                        let byte_tokens: Option<Vec<_>> = text
                            .bytes()
                            .map(|byte| {
                                let value = format!("<0x{byte:02X}>");
                                let id = self.trie.get(value.as_bytes())?;
                                Some(Token::new(id, value, (start, end)))
                            })
                            .collect();
                        if let Some(byte_tokens) = byte_tokens {
                            tokens.extend(byte_tokens);
                            continue;
                        }
                    }
                    self.unk_id.ok_or_else(Self::missing_unknown)? as u32
                }
            };
            tokens.push(Token::new(id, text.to_owned(), (start, end)));
        }
        Ok(tokens)
    }

    fn token_to_id(&self, token: &str) -> Option<u32> {
        self.trie.get(token.as_bytes())
    }

    fn id_to_token(&self, id: u32) -> Option<String> {
        self.piece(id as usize).map(str::to_owned)
    }

    fn get_vocab(&self) -> HashMap<String, u32> {
        (0..self.piece_ends.len())
            .filter_map(|id| Some((self.piece(id)?.to_owned(), id as u32)))
            .collect()
    }

    fn get_vocab_size(&self) -> usize {
        self.piece_ends.len()
    }

    fn save(&self, _folder: &Path, _prefix: Option<&str>) -> Result<Vec<PathBuf>> {
        Err("The compact Unigram model is read-only".into())
    }

    fn get_trainer(&self) -> UnigramTrainer {
        UnigramTrainer::default()
    }
}

impl<'de> Deserialize<'de> for CompactUnigram {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        deserializer.deserialize_struct(
            "Unigram",
            &["type", "vocab", "unk_id", "byte_fallback"],
            ModelVisitor,
        )
    }
}

struct ModelVisitor;

impl<'de> Visitor<'de> for ModelVisitor {
    type Value = CompactUnigram;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct Unigram")
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut pieces = None;
        let mut unk_id: Option<usize> = None;
        let mut byte_fallback = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "type" => {
                    let kind: String = map.next_value()?;
                    if kind != "Unigram" {
                        return Err(de::Error::invalid_value(Unexpected::Str(&kind), &"Unigram"));
                    }
                }
                "unk_id" => unk_id = map.next_value()?,
                "byte_fallback" => byte_fallback = map.next_value()?,
                "vocab" => pieces = Some(map.next_value_seed(VocabularySeed)?),
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        let pieces = pieces.ok_or_else(|| de::Error::custom("Missing vocab"))?;
        CompactUnigram::new(pieces, unk_id, byte_fallback)
            .map_err(|error| de::Error::custom(format!("Unable to load vocab {error:?}")))
    }
}

struct VocabularySeed;

impl<'de> DeserializeSeed<'de> for VocabularySeed {
    type Value = Pieces;

    fn deserialize<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Pieces, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for VocabularySeed {
    type Value = Pieces;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a sequence of [piece, score] pairs")
    }

    fn visit_seq<A: SeqAccess<'de>>(
        self,
        mut sequence: A,
    ) -> std::result::Result<Pieces, A::Error> {
        let mut pieces = Pieces::default();
        while sequence
            .next_element_seed(EntrySeed(&mut pieces))?
            .is_some()
        {}
        Ok(pieces)
    }
}

struct EntrySeed<'a>(&'a mut Pieces);

impl<'de> DeserializeSeed<'de> for EntrySeed<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        deserializer.deserialize_tuple(2, self)
    }
}

impl<'de> Visitor<'de> for EntrySeed<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a [piece, score] pair")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> std::result::Result<(), A::Error> {
        sequence
            .next_element_seed(PieceSeed(&mut self.0.text))?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let score: f64 = sequence
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::invalid_length(3, &self));
        }
        let end = u32::try_from(self.0.text.len())
            .ok()
            .filter(|end| *end < u32::MAX)
            .ok_or_else(|| de::Error::custom("The vocabulary is too large"))?;
        self.0.ends.push(end);
        self.0.scores.push(score);
        Ok(())
    }
}

struct PieceSeed<'a>(&'a mut String);

impl<'de> DeserializeSeed<'de> for PieceSeed<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        deserializer.deserialize_str(self)
    }
}

impl Visitor<'_> for PieceSeed<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string piece")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<(), E> {
        self.0.push_str(value);
        Ok(())
    }
}
