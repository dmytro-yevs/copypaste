//! Snippet import: parsing, planning and commit.
//!
//! [`read`] streams a bundle (sealed or plain), a JSON array of
//! `{name, text, keyword}` objects or a CSV file into bounded rows; foreign
//! rows arrive with Template off unless known placeholders are converted.
//! [`plan`] classifies every row against one snapshot of the library without
//! writing. [`commit`] re-checks each row and writes them all in one
//! transaction under the chosen policies, so peers receive one batch and a
//! failure or a stop midway writes nothing. Rows are zeroized when dropped.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufReader, Read};
use std::marker::PhantomData;
use std::path::Path;

use copypaste_ipc::snippets::{
    AbbreviationConflictPolicy, CollectionNamePolicy, ImportCounts, ImportFormat, ImportPolicies,
    ImportReport, ImportRow, ImportRowOutcome, SameIdPolicy,
};
use serde::de::{self, DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use unicode_segmentation::UnicodeSegmentation;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::abbreviation;
use super::bundle::{
    self, BundleCollection, BundleSnippet, OpenReader, TransferError, DEFAULT_COLLECTION_ACCENT,
    DEFAULT_COLLECTION_ICON, FORMAT, MAX_BUNDLE_COLLECTIONS, MAX_IMPORT_BYTES, MAX_IMPORT_ROWS,
    VERSION,
};
use crate::storage::organization::SnippetCollectionRecord;
use crate::storage::snippets::{
    CollectionDraft, SnippetBatch, SnippetDraft, SnippetError, SnippetReader, MAX_BODY_BYTES,
    MAX_COLLECTION_NAME_CHARS, MAX_SNIPPETS, MAX_TITLE_CHARS,
};
use crate::storage::StoreError;
use crate::{ItemKey, Store};

/// Reasons a preview row carries: stable codes the UI translates.
pub const REASON_INVALID_TITLE: &str = "invalid_title";
pub const REASON_EMPTY_BODY: &str = "empty_body";
pub const REASON_TOO_LARGE: &str = "too_large";
/// A CSV record that is not valid UTF-8.
pub const REASON_MALFORMED: &str = "malformed";
pub const REASON_LIBRARY_FULL: &str = "library_full";
pub const REASON_ABBREVIATION_TAKEN: &str = "abbreviation_taken";
pub const REASON_INVALID_ABBREVIATION: &str = "invalid_abbreviation";
/// Followed by `:` and the placeholders left literal, comma separated.
pub const REASON_UNRECOGNIZED: &str = "unrecognized_placeholders";
/// Appended to the title of a "Keep both" copy.
pub const IMPORTED_SUFFIX: &str = " (imported)";

/// Unrecognized placeholders listed per row, and the length kept of each.
const MAX_UNRECOGNIZED: usize = 10;
const MAX_TOKEN_CHARS: usize = 64;
/// A foreign placeholder is short; a longer brace run is left alone.
const MAX_TOKEN_BYTES: usize = 200;
const RECOGNIZED: [&str; 6] = ["clipboard", "cursor", "date", "time", "datetime", "uuid"];
/// `abbr-2` … `abbr-999` are tried before an abbreviation is dropped.
const MAX_SUFFIX: u32 = 999;

#[derive(Default, Zeroize, ZeroizeOnDrop)]
struct SourceCollection {
    /// What rows name it by: the bundle id, or the folded CSV name.
    key: String,
    /// A bundle id, kept when no local collection ever had it.
    id: Option<String>,
    name: String,
    icon: Option<String>,
    accent: Option<String>,
}

#[derive(Default, Zeroize, ZeroizeOnDrop)]
struct SourceRow {
    id: Option<String>,
    /// The key of a [`SourceCollection`].
    collection: Option<String>,
    title: String,
    abbreviation: Option<String>,
    body_text: String,
    template: bool,
    pinned: bool,
    concealed: bool,
    unrecognized: Vec<String>,
    #[zeroize(skip)]
    problem: Option<&'static str>,
}

/// A parsed file, not yet compared with the library.
#[derive(Default)]
pub struct ParsedImport {
    collections: Vec<SourceCollection>,
    rows: Vec<SourceRow>,
}

impl ParsedImport {
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// A previewed import: the parsed rows and what each would do now.
pub struct ImportPlan {
    token: String,
    parsed: ParsedImport,
    outcomes: Vec<(ImportRowOutcome, Option<String>)>,
    counts: ImportCounts,
}

impl ImportPlan {
    /// Random and unguessable; names this plan until it is used or dropped.
    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn counts(&self) -> ImportCounts {
        self.counts
    }

    /// Every row that is not plainly new, or that carries a note.
    pub fn rows(&self) -> Vec<ImportRow> {
        self.outcomes
            .iter()
            .enumerate()
            .filter(|(_, (outcome, reason))| *outcome != ImportRowOutcome::New || reason.is_some())
            .map(|(index, (outcome, reason))| ImportRow {
                index: count(index),
                outcome: *outcome,
                reason: reason.clone(),
            })
            .collect()
    }
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Opens and parses an import file of `format`.
pub fn read(
    path: &Path,
    passphrase: Option<&str>,
    format: ImportFormat,
    convert_placeholders: bool,
) -> Result<ParsedImport, TransferError> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > MAX_IMPORT_BYTES + bundle::SEALED_OVERHEAD {
        return Err(TransferError::TooLarge);
    }
    parse(
        BufReader::new(file),
        passphrase,
        format,
        convert_placeholders,
    )
}

/// [`read`] over any reader.
pub fn parse(
    mut reader: impl BufRead,
    passphrase: Option<&str>,
    format: ImportFormat,
    convert_placeholders: bool,
) -> Result<ParsedImport, TransferError> {
    if format == ImportFormat::Bundle && bundle::is_sealed(reader.fill_buf()?) {
        let passphrase = passphrase.ok_or(TransferError::PassphraseRequired)?;
        let mut opened = OpenReader::new(reader, passphrase)?;
        let parsed = parse_plain(&mut opened, format, convert_placeholders);
        return parsed.map_err(|error| opened.failure.take().unwrap_or(error));
    }
    parse_plain(reader, format, convert_placeholders)
}

/// Refuses to read past [`MAX_IMPORT_BYTES`] and remembers that it did.
struct Capped<R> {
    inner: R,
    left: u64,
    exceeded: bool,
}

impl<R: Read> Read for Capped<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let room = usize::try_from(self.left.saturating_add(1)).unwrap_or(usize::MAX);
        let wanted = out.len().min(room);
        let read = self.inner.read(&mut out[..wanted])?;
        if read as u64 > self.left {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "import too large",
            ));
        }
        self.left -= read as u64;
        Ok(read)
    }
}

fn parse_plain(
    reader: impl Read,
    format: ImportFormat,
    convert: bool,
) -> Result<ParsedImport, TransferError> {
    let mut capped = Capped {
        inner: reader,
        left: MAX_IMPORT_BYTES,
        exceeded: false,
    };
    let parsed = match format {
        ImportFormat::Bundle => parse_bundle(BufReader::new(&mut capped)),
        ImportFormat::Json => parse_json(BufReader::new(&mut capped), convert),
        ImportFormat::Csv => parse_csv(BufReader::new(&mut capped), convert),
    };
    if capped.exceeded {
        return Err(TransferError::TooLarge);
    }
    let mut parsed = parsed?;
    let mut ids = HashSet::new();
    for row in &mut parsed.rows {
        if row.id.as_ref().is_some_and(|id| !ids.insert(id.clone())) {
            row.id = None;
        }
    }
    Ok(parsed)
}

fn json_error(error: serde_json::Error) -> TransferError {
    if error.is_io() {
        TransferError::Io(error.into())
    } else {
        TransferError::Malformed
    }
}

/// Feeds each array element to `each`, failing past `limit` elements, so a
/// file of many tiny rows never grows a large vector first.
struct Elements<'a, T, F> {
    limit: usize,
    overflow: &'a Cell<bool>,
    each: F,
    element: PhantomData<fn() -> T>,
}

impl<'a, T, F> Elements<'a, T, F> {
    fn new(limit: usize, overflow: &'a Cell<bool>, each: F) -> Self {
        Self {
            limit,
            overflow,
            each,
            element: PhantomData,
        }
    }
}

impl<'de, T: Deserialize<'de>, F: FnMut(T)> DeserializeSeed<'de> for Elements<'_, T, F> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de, T: Deserialize<'de>, F: FnMut(T)> Visitor<'de> for Elements<'_, T, F> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an array")
    }

    fn visit_seq<A: SeqAccess<'de>>(mut self, mut seq: A) -> Result<(), A::Error> {
        let mut seen = 0;
        while let Some(element) = seq.next_element::<T>()? {
            if seen == self.limit {
                self.overflow.set(true);
                return Err(de::Error::custom("too many elements"));
            }
            seen += 1;
            (self.each)(element);
        }
        Ok(())
    }
}

#[derive(Default)]
struct BundleHead {
    format: Option<String>,
    version: Option<u64>,
}

struct BundleVisitor<'a> {
    head: &'a mut BundleHead,
    parsed: &'a mut ParsedImport,
    overflow: &'a Cell<bool>,
}

impl<'de> Visitor<'de> for BundleVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a snippet bundle")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "format" => self.head.format = Some(map.next_value()?),
                "version" => self.head.version = Some(map.next_value()?),
                "collections" => {
                    let collections = &mut self.parsed.collections;
                    map.next_value_seed(Elements::new(
                        MAX_BUNDLE_COLLECTIONS,
                        self.overflow,
                        |mut collection: BundleCollection| {
                            collections.push(bundle_collection(&mut collection));
                        },
                    ))?;
                }
                "snippets" => {
                    let rows = &mut self.parsed.rows;
                    map.next_value_seed(Elements::new(
                        MAX_IMPORT_ROWS,
                        self.overflow,
                        |mut snippet: BundleSnippet| rows.push(bundle_row(&mut snippet)),
                    ))?;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(())
    }
}

/// A stable id in the shape every transport accepts, or `None`.
fn portable_id(id: &str) -> Option<String> {
    (!id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'))
    .then(|| id.to_owned())
}

/// Single-line text of at most `max_chars`: control characters become
/// spaces and the rest is cut.
fn one_line(text: &str, max_chars: usize) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn bundle_collection(collection: &mut BundleCollection) -> SourceCollection {
    SourceCollection {
        key: collection.id.clone(),
        id: portable_id(&collection.id),
        name: one_line(&collection.name, MAX_COLLECTION_NAME_CHARS),
        icon: collection.icon.take(),
        accent: collection.accent.take(),
    }
}

fn bundle_row(snippet: &mut BundleSnippet) -> SourceRow {
    let mut row = SourceRow::default();
    row.id = portable_id(&snippet.id);
    row.collection = snippet.collection_id.take();
    row.title = snippet.title.trim().to_owned();
    row.abbreviation = snippet.abbreviation.take();
    row.body_text = std::mem::take(&mut snippet.body_text);
    row.template = snippet.template;
    row.pinned = snippet.pinned;
    row.concealed = snippet.concealed;
    finish_row(&mut row);
    row
}

/// Line endings and empty abbreviations as the store keeps them, and the
/// problem that makes the row invalid, if any.
fn finish_row(row: &mut SourceRow) {
    if row.body_text.contains('\r') {
        row.body_text = row.body_text.replace("\r\n", "\n").replace('\r', "\n");
    }
    row.abbreviation = row
        .abbreviation
        .take()
        .map(|abbreviation| abbreviation.trim().to_owned())
        .filter(|abbreviation| !abbreviation.is_empty());
    row.problem = row.problem.or(
        if row.title.chars().count() > MAX_TITLE_CHARS || row.title.chars().any(char::is_control) {
            Some(REASON_INVALID_TITLE)
        } else if row.body_text.trim().is_empty() {
            Some(REASON_EMPTY_BODY)
        } else if row.body_text.len() > MAX_BODY_BYTES {
            Some(REASON_TOO_LARGE)
        } else {
            None
        },
    );
}

fn parse_bundle(reader: impl Read) -> Result<ParsedImport, TransferError> {
    let overflow = Cell::new(false);
    let mut parsed = ParsedImport::default();
    let mut head = BundleHead::default();
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let result = deserializer
        .deserialize_map(BundleVisitor {
            head: &mut head,
            parsed: &mut parsed,
            overflow: &overflow,
        })
        .and_then(|()| deserializer.end());
    if overflow.get() {
        return Err(TransferError::TooManyRows);
    }
    result.map_err(json_error)?;
    if head.format.as_deref() != Some(FORMAT) {
        return Err(TransferError::Malformed);
    }
    match head.version {
        Some(version) if version == u64::from(VERSION) => Ok(parsed),
        Some(version) if version > u64::from(VERSION) => Err(TransferError::UnsupportedVersion),
        _ => Err(TransferError::Malformed),
    }
}

/// The shape other launchers export: `{name, text, keyword}`.
#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
struct ForeignSnippet {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    keyword: Option<String>,
}

fn parse_json(reader: impl Read, convert: bool) -> Result<ParsedImport, TransferError> {
    let overflow = Cell::new(false);
    let mut parsed = ParsedImport::default();
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let rows = &mut parsed.rows;
    let result = Elements::new(MAX_IMPORT_ROWS, &overflow, |mut snippet: ForeignSnippet| {
        rows.push(foreign_row(
            snippet.name.as_deref().unwrap_or_default(),
            snippet.keyword.take(),
            snippet.text.as_deref(),
            None,
            convert,
        ));
    })
    .deserialize(&mut deserializer)
    .and_then(|()| deserializer.end());
    if overflow.get() {
        return Err(TransferError::TooManyRows);
    }
    result.map_err(json_error)?;
    Ok(parsed)
}

fn csv_error(error: csv::Error) -> TransferError {
    match error.into_kind() {
        csv::ErrorKind::Io(error) => TransferError::Io(error),
        _ => TransferError::Malformed,
    }
}

/// CSV with a header naming a body column and, optionally, title,
/// abbreviation and collection columns.
fn parse_csv(reader: impl Read, convert: bool) -> Result<ParsedImport, TransferError> {
    let mut csv = csv::ReaderBuilder::new().flexible(true).from_reader(reader);
    let headers: Vec<String> = csv
        .headers()
        .map_err(csv_error)?
        .iter()
        .map(|header| header.trim().to_lowercase())
        .collect();
    let column = |names: &[&str]| headers.iter().position(|h| names.contains(&h.as_str()));
    let body =
        column(&["body", "text", "snippet", "content"]).ok_or(TransferError::MissingColumns)?;
    let title = column(&["title", "name"]);
    let abbreviation = column(&["abbreviation", "keyword"]);
    let collection = column(&["collection", "folder", "group"]);

    let mut parsed = ParsedImport::default();
    let mut record = csv::StringRecord::new();
    loop {
        match csv.read_record(&mut record) {
            Ok(false) => break,
            Ok(true) => {
                let field = |index: Option<usize>| index.and_then(|index| record.get(index));
                let collection = field(collection)
                    .map(|name| one_line(name, MAX_COLLECTION_NAME_CHARS))
                    .filter(|name| !name.is_empty())
                    .map(|name| named_collection(&mut parsed.collections, name));
                let row = foreign_row(
                    field(title).unwrap_or_default(),
                    field(abbreviation).map(str::to_owned),
                    field(Some(body)),
                    collection,
                    convert,
                );
                parsed.rows.push(row);
            }
            Err(error) if matches!(error.kind(), csv::ErrorKind::Utf8 { .. }) => {
                let mut row = SourceRow::default();
                row.problem = Some(REASON_MALFORMED);
                parsed.rows.push(row);
            }
            Err(error) => return Err(csv_error(error)),
        }
        if parsed.rows.len() > MAX_IMPORT_ROWS {
            return Err(TransferError::TooManyRows);
        }
    }
    record.clear();
    Ok(parsed)
}

/// The key of the CSV collection called `name`, added on first use.
fn named_collection(collections: &mut Vec<SourceCollection>, name: String) -> String {
    let key = abbreviation::normalize(&name);
    if !collections.iter().any(|collection| collection.key == key) {
        let mut collection = SourceCollection::default();
        collection.key = key.clone();
        collection.name = name;
        collections.push(collection);
    }
    key
}

fn foreign_row(
    title: &str,
    abbreviation: Option<String>,
    body: Option<&str>,
    collection: Option<String>,
    convert: bool,
) -> SourceRow {
    let body = body.unwrap_or_default();
    let mut row = SourceRow::default();
    row.collection = collection;
    row.title = one_line(title, MAX_TITLE_CHARS + 1);
    row.abbreviation = abbreviation;
    if convert {
        let converted = convert_placeholders(body);
        row.body_text = converted.text;
        row.template = converted.template;
        row.unrecognized = converted.unrecognized;
    } else {
        row.body_text = body.to_owned();
    }
    finish_row(&mut row);
    row
}

/// A foreign body with its known placeholders in CopyPaste syntax.
pub struct Converted {
    pub text: String,
    /// On only when a placeholder was converted; literal braces are then
    /// escaped so they stay literal.
    pub template: bool,
    /// Placeholders left literal, at most ten, each cut to 64 characters.
    pub unrecognized: Vec<String>,
}

/// Translates `{clipboard}`, `{cursor}`, `{date}`, `{time}`, `{datetime}`
/// and `{uuid}` into placeholders and lists other `{name …}` tokens.
pub fn convert_placeholders(text: &str) -> Converted {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len() + 16);
    let mut unrecognized = Vec::new();
    let mut converted = false;
    let mut literal = 0;
    let mut at = 0;
    while at < bytes.len() {
        let doubled = bytes.get(at + 1) == Some(&bytes[at]);
        match bytes[at] {
            b'{' | b'}' if doubled => {
                out.push_str(&text[literal..at]);
                out.push('\\');
                out.push_str(&text[at..at + 2]);
                at += 2;
                literal = at;
            }
            b'{' => match token_end(bytes, at) {
                Some(end) => {
                    let inner = text[at + 1..end].trim();
                    if let Some(name) = RECOGNIZED
                        .iter()
                        .find(|name| inner.eq_ignore_ascii_case(name))
                    {
                        out.push_str(&text[literal..at]);
                        out.push_str("{{");
                        out.push_str(name);
                        out.push_str("}}");
                        converted = true;
                        literal = end + 1;
                    } else {
                        let token: String = text[at..=end].chars().take(MAX_TOKEN_CHARS).collect();
                        if unrecognized.len() < MAX_UNRECOGNIZED && !unrecognized.contains(&token) {
                            unrecognized.push(token);
                        }
                    }
                    at = end + 1;
                }
                None => at += 1,
            },
            _ => at += 1,
        }
    }
    if !converted {
        return Converted {
            text: text.to_owned(),
            template: false,
            unrecognized,
        };
    }
    out.push_str(&text[literal..]);
    Converted {
        text: out,
        template: true,
        unrecognized,
    }
}

/// The closing brace of a `{name}` or `{name args}` token at `start`.
fn token_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut at = start + 1;
    if !bytes.get(at)?.is_ascii_alphabetic() {
        return None;
    }
    while bytes
        .get(at)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
    {
        at += 1;
    }
    if *bytes.get(at)? != b'}' && *bytes.get(at)? != b' ' {
        return None;
    }
    while at - start < MAX_TOKEN_BYTES {
        match *bytes.get(at)? {
            b'}' => return Some(at),
            b'{' | b'\n' | b'\r' => return None,
            _ => at += 1,
        }
    }
    None
}

impl SourceRow {
    fn note(&self) -> Option<String> {
        (!self.unrecognized.is_empty())
            .then(|| format!("{REASON_UNRECOGNIZED}:{}", self.unrecognized.join(", ")))
    }
}

/// What a row would do against the library as it is now. While planning,
/// `seen` holds the abbreviations of earlier rows of the same file.
fn classify(
    reader: &SnippetReader<'_>,
    row: &SourceRow,
    seen: Option<&mut HashSet<String>>,
) -> Result<(ImportRowOutcome, Option<String>), StoreError> {
    if let Some(problem) = row.problem {
        return Ok((ImportRowOutcome::Invalid, Some(problem.to_owned())));
    }
    if let Some(id) = &row.id {
        if let Some((record, body)) = reader.snippet(id)?.filter(|(record, _)| !record.purged) {
            let identical = record.title == row.title
                && record.abbreviation == row.abbreviation
                && record.template == row.template
                && record.concealed == row.concealed
                && body.is_some_and(|body| body.text == row.body_text);
            return Ok(if identical {
                (ImportRowOutcome::Identical, None)
            } else {
                (ImportRowOutcome::Updated, row.note())
            });
        }
    }
    let Some(wanted) = row.abbreviation.as_deref() else {
        return Ok((ImportRowOutcome::New, row.note()));
    };
    if abbreviation::validate(wanted).is_err() {
        return Ok((
            ImportRowOutcome::New,
            Some(REASON_INVALID_ABBREVIATION.to_owned()),
        ));
    }
    let key = abbreviation::normalize(wanted);
    let mut taken = reader.abbreviation_owner(wanted)?.is_some();
    if let Some(seen) = seen {
        taken |= !seen.insert(key);
    }
    Ok(if taken {
        (
            ImportRowOutcome::AbbreviationConflict,
            Some(REASON_ABBREVIATION_TAKEN.to_owned()),
        )
    } else {
        (ImportRowOutcome::New, row.note())
    })
}

/// Whether a local collection already stands for `source` (by id, or by
/// name when collections are merged).
fn existing_collection(
    live: &[SnippetCollectionRecord],
    source: &SourceCollection,
    merge: bool,
) -> Option<String> {
    if let Some(found) = source
        .id
        .as_ref()
        .and_then(|id| live.iter().find(|collection| &collection.id == id))
    {
        return Some(found.id.clone());
    }
    let folded = abbreviation::normalize(&source.name);
    live.iter()
        .find(|collection| merge && abbreviation::normalize(&collection.name) == folded)
        .map(|collection| collection.id.clone())
}

/// Classifies every row against one snapshot of the library; writes nothing.
pub fn plan(store: &Store, parsed: ParsedImport) -> Result<ImportPlan, TransferError> {
    let (outcomes, counts) = store.read_snippets(|reader| {
        let mut counts = ImportCounts::default();
        let mut seen = HashSet::new();
        let mut room = MAX_SNIPPETS.saturating_sub(reader.snippet_total()?);
        let live = reader.collections()?;
        let sources: HashMap<&str, &SourceCollection> = parsed
            .collections
            .iter()
            .map(|collection| (collection.key.as_str(), collection))
            .collect();
        let mut created = HashSet::new();
        let mut outcomes = Vec::with_capacity(parsed.rows.len());
        for row in &parsed.rows {
            let (mut outcome, mut reason) = classify(reader, row, Some(&mut seen))?;
            if !matches!(
                outcome,
                ImportRowOutcome::Invalid | ImportRowOutcome::Identical
            ) {
                if room == 0 {
                    outcome = ImportRowOutcome::Invalid;
                    reason = Some(REASON_LIBRARY_FULL.to_owned());
                } else {
                    room -= 1;
                    if let Some(source) = row.collection.as_deref().and_then(|key| sources.get(key))
                    {
                        if existing_collection(&live, source, true).is_none() {
                            created.insert(source.key.as_str());
                        }
                    }
                }
            }
            let tally = match outcome {
                ImportRowOutcome::New => &mut counts.new,
                ImportRowOutcome::Identical => &mut counts.identical,
                ImportRowOutcome::Updated => &mut counts.updated,
                ImportRowOutcome::AbbreviationConflict => &mut counts.abbreviation_conflicts,
                ImportRowOutcome::Invalid => &mut counts.invalid,
            };
            *tally += 1;
            outcomes.push((outcome, reason));
        }
        counts.collections = count(created.len());
        Ok((outcomes, counts))
    })?;
    Ok(ImportPlan {
        token: uuid::Uuid::new_v4().simple().to_string(),
        parsed,
        outcomes,
        counts,
    })
}

/// Writes a planned import in one transaction. `stop` is asked before each
/// row; once it answers true nothing is written and the result is
/// [`TransferError::Cancelled`].
pub fn commit(
    store: &Store,
    key: &ItemKey,
    plan: ImportPlan,
    policies: ImportPolicies,
    stop: &dyn Fn() -> bool,
) -> Result<ImportReport, TransferError> {
    let ImportPlan {
        parsed, outcomes, ..
    } = plan;
    let stopped = Cell::new(false);
    let written = store.write_snippets(key, |batch| {
        let mut writer = Writer {
            batch,
            policies,
            sources: parsed
                .collections
                .iter()
                .map(|collection| (collection.key.as_str(), collection))
                .collect(),
            targets: HashMap::new(),
            report: ImportReport::default(),
        };
        for (row, (planned, _)) in parsed.rows.iter().zip(&outcomes) {
            if stop() {
                stopped.set(true);
                return Err(SnippetError::NotFound);
            }
            if *planned == ImportRowOutcome::Invalid {
                writer.report.skipped += 1;
                continue;
            }
            writer.write(row)?;
        }
        Ok(writer.report)
    });
    if stopped.get() {
        return Err(TransferError::Cancelled);
    }
    Ok(written?)
}

struct Writer<'a, 'b> {
    batch: &'a SnippetBatch<'b>,
    policies: ImportPolicies,
    sources: HashMap<&'a str, &'a SourceCollection>,
    /// Collection keys already resolved to a local collection or Unfiled.
    targets: HashMap<String, Option<String>>,
    report: ImportReport,
}

impl Writer<'_, '_> {
    fn write(&mut self, row: &SourceRow) -> Result<(), SnippetError> {
        let (outcome, _) = classify(self.batch, row, None)?;
        let mut draft = SnippetDraft {
            title: row.title.clone(),
            abbreviation: row.abbreviation.clone(),
            body_text: row.body_text.clone(),
            template: row.template,
            pinned: row.pinned,
            concealed: row.concealed,
            ..SnippetDraft::default()
        };
        let mut new_id = row.id.as_deref();
        let replace = match outcome {
            ImportRowOutcome::Invalid | ImportRowOutcome::Identical => {
                self.report.skipped += 1;
                return Ok(());
            }
            ImportRowOutcome::Updated => match self.policies.same_id_different {
                SameIdPolicy::Skip => {
                    self.report.skipped += 1;
                    return Ok(());
                }
                SameIdPolicy::Replace => {
                    draft.id = row.id.clone();
                    draft.base_revision = row
                        .id
                        .as_deref()
                        .map(|id| self.batch.snippet(id))
                        .transpose()?
                        .flatten()
                        .and_then(|(_, body)| body)
                        .map(|body| body.revision.clone());
                    true
                }
                SameIdPolicy::KeepBoth => {
                    new_id = None;
                    draft.title = imported_title(&row.title);
                    false
                }
            },
            ImportRowOutcome::New | ImportRowOutcome::AbbreviationConflict => false,
        };
        draft.collection_id = self.collection(row.collection.as_deref())?;
        let mut renamed = false;
        let mut dropped = false;
        if draft
            .abbreviation
            .as_deref()
            .is_some_and(|wanted| abbreviation::validate(wanted).is_err())
        {
            draft.abbreviation = None;
            dropped = true;
        }
        loop {
            match self.batch.save(&draft, new_id) {
                Ok(_) => break,
                Err(SnippetError::AbbreviationTaken { .. }) => {
                    let taken = draft.abbreviation.take().unwrap_or_default();
                    match self.policies.abbreviation_conflict {
                        AbbreviationConflictPolicy::Skip => {
                            self.report.skipped += 1;
                            return Ok(());
                        }
                        AbbreviationConflictPolicy::ImportWithout => dropped = true,
                        AbbreviationConflictPolicy::RenameWithSuffix => {
                            draft.abbreviation = self.free_abbreviation(&taken)?;
                            renamed = draft.abbreviation.is_some();
                            dropped = !renamed;
                        }
                    }
                }
                Err(
                    SnippetError::EmptyBody
                    | SnippetError::TooLarge
                    | SnippetError::InvalidTitle
                    | SnippetError::InvalidAbbreviation(_),
                ) => {
                    self.report.skipped += 1;
                    return Ok(());
                }
                Err(error) => return Err(error),
            }
        }
        let report = &mut self.report;
        if replace {
            report.updated += 1;
        } else {
            report.inserted += 1;
        }
        report.renamed_abbreviations += u32::from(renamed);
        report.without_abbreviation += u32::from(dropped);
        report.unrecognized_placeholders += count(row.unrecognized.len());
        Ok(())
    }

    /// `abbr-2`, `abbr-3`, … (shortened to fit) until one is free.
    fn free_abbreviation(&self, taken: &str) -> Result<Option<String>, StoreError> {
        for suffix in 2..=MAX_SUFFIX {
            let suffix = format!("-{suffix}");
            let keep = abbreviation::MAX_GRAPHEMES - suffix.len();
            let candidate: String = taken.graphemes(true).take(keep).collect::<String>() + &suffix;
            if self.batch.abbreviation_owner(&candidate)?.is_none() {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }

    /// The local collection rows of `key` go to, created on first use;
    /// Unfiled when none can be made.
    fn collection(&mut self, key: Option<&str>) -> Result<Option<String>, SnippetError> {
        let Some(source) = key.and_then(|key| self.sources.get(key).copied()) else {
            return Ok(None);
        };
        if let Some(target) = self.targets.get(&source.key) {
            return Ok(target.clone());
        }
        let merge = self.policies.collection_name == CollectionNamePolicy::Merge;
        let live = self.batch.collections()?;
        let target = match existing_collection(&live, source, merge) {
            Some(id) => Some(id),
            None => {
                let draft = CollectionDraft {
                    id: None,
                    name: free_collection_name(&live, &source.name),
                    icon: style(source.icon.as_deref(), DEFAULT_COLLECTION_ICON),
                    accent: style(source.accent.as_deref(), DEFAULT_COLLECTION_ACCENT),
                };
                match self.batch.save_collection(&draft, source.id.as_deref()) {
                    Ok(id) => {
                        self.report.collections_created += 1;
                        Some(id)
                    }
                    Err(SnippetError::CollectionNameTaken { collection_id }) => Some(collection_id),
                    Err(SnippetError::LimitReached | SnippetError::InvalidCollection) => None,
                    Err(error) => return Err(error),
                }
            }
        };
        self.targets.insert(source.key.clone(), target.clone());
        Ok(target)
    }
}

fn style(value: Option<&str>, default: &str) -> String {
    value
        .map(|value| one_line(value, 64))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

/// `name`, or `name (2)`, `name (3)`, … when a live collection has it.
fn free_collection_name(live: &[SnippetCollectionRecord], name: &str) -> String {
    let taken = |candidate: &str| {
        let folded = abbreviation::normalize(candidate);
        live.iter()
            .any(|collection| abbreviation::normalize(&collection.name) == folded)
    };
    if !taken(name) {
        return name.to_owned();
    }
    (2..=live.len() + 2)
        .map(|suffix| {
            let suffix = format!(" ({suffix})");
            let keep = MAX_COLLECTION_NAME_CHARS - suffix.chars().count();
            name.chars().take(keep).collect::<String>() + &suffix
        })
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| name.to_owned())
}

fn imported_title(title: &str) -> String {
    let keep = MAX_TITLE_CHARS - IMPORTED_SUFFIX.chars().count();
    let title: String = title.chars().take(keep).collect();
    (title + IMPORTED_SUFFIX).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_foreign_placeholders_convert_and_literal_braces_stay_literal() {
        let converted =
            convert_placeholders("Hi {clipboard} at {Date}, {{x}} {argument name=\"a\"} {cursor}");
        assert!(converted.template);
        assert_eq!(
            converted.text,
            r#"Hi {{clipboard}} at {{date}}, \{{x\}} {argument name="a"} {{cursor}}"#
        );
        assert_eq!(converted.unrecognized, vec![r#"{argument name="a"}"#]);

        let untouched = convert_placeholders("code { x } and {unknown}");
        assert!(!untouched.template);
        assert_eq!(untouched.text, "code { x } and {unknown}");
        assert_eq!(untouched.unrecognized, vec!["{unknown}"]);
    }

    #[test]
    fn csv_rows_need_a_body_column_and_fold_collection_names() {
        let csv = "Title,Keyword,Body,Collection\nA,a1,first,Work\nB,,second,work\nC,,,\n";
        let parsed = parse(csv.as_bytes(), None, ImportFormat::Csv, false).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed.collections.len(), 1);
        assert_eq!(parsed.rows[1].collection, parsed.rows[0].collection);
        assert_eq!(parsed.rows[2].problem, Some(REASON_EMPTY_BODY));

        assert!(matches!(
            parse(
                "Title,Keyword\nA,b\n".as_bytes(),
                None,
                ImportFormat::Csv,
                false
            ),
            Err(TransferError::MissingColumns)
        ));
    }
}
