//! Lays out reader events as indented or compact JSON, and reorders object
//! members by key.

use std::borrow::Cow;

use super::reader::{Container, Event, Sink};
use super::string::decode_key;
use crate::catalogue::Context;
use crate::error::{check_output_len, TransformError};

/// Writes events as JSON: one member or element per line indented by
/// `unit`, or compact without any whitespace. Several top-level values are
/// written one per line.
pub(crate) struct Writer {
    out: String,
    unit: Option<&'static str>,
    counts: Vec<usize>,
    containers: Vec<Container>,
    documents: usize,
}

impl Writer {
    pub(crate) fn new(unit: Option<&'static str>, capacity: usize) -> Self {
        Self {
            out: String::with_capacity(capacity),
            unit,
            counts: Vec::new(),
            containers: Vec::new(),
            documents: 0,
        }
    }

    pub(crate) fn finish(self) -> String {
        self.out
    }

    fn newline(&mut self) {
        if let Some(unit) = self.unit {
            self.out.push('\n');
            for _ in 0..self.containers.len() {
                self.out.push_str(unit);
            }
        }
    }

    fn item(&mut self) {
        if let Some(count) = self.counts.last_mut() {
            if *count > 0 {
                self.out.push(',');
            }
            *count += 1;
        }
        self.newline();
    }

    fn begin_value(&mut self) {
        match self.containers.last() {
            None if self.documents > 0 => self.out.push('\n'),
            Some(Container::Array) => self.item(),
            _ => {}
        }
    }

    fn end_value(&mut self) {
        if self.containers.is_empty() {
            self.documents += 1;
        }
    }
}

impl Sink for Writer {
    fn event(&mut self, event: Event<'_>) -> Result<(), TransformError> {
        match event {
            Event::Open(container) => {
                self.begin_value();
                self.out.push(match container {
                    Container::Object => '{',
                    Container::Array => '[',
                });
                self.containers.push(container);
                self.counts.push(0);
            }
            Event::Close(container) => {
                self.containers.pop();
                if self.counts.pop().is_some_and(|count| count > 0) {
                    self.newline();
                }
                self.out.push(match container {
                    Container::Object => '}',
                    Container::Array => ']',
                });
                self.end_value();
            }
            Event::Key(raw) => {
                self.item();
                self.out.push_str(raw);
                self.out
                    .push_str(if self.unit.is_some() { ": " } else { ":" });
            }
            Event::Value(raw) => {
                self.begin_value();
                self.out.push_str(raw);
                self.end_value();
            }
        }
        check_output_len(self.out.len())
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Open(Container),
    Close(Container),
    Key,
    Value,
}

/// One buffered event in 12 bytes: a token's offset and length in the
/// text, or for an opening bracket the index just past its value.
#[derive(Clone, Copy)]
struct Entry {
    kind: Kind,
    a: u32,
    b: u32,
}

/// Collects events, then replays them with object members ordered by
/// decoded key in code point order. Equal keys keep their order; arrays are
/// never reordered. Without `recursive` only top-level objects are sorted.
pub(crate) struct Sorter<'a> {
    text: &'a str,
    entries: Vec<Entry>,
    open: Vec<usize>,
    recursive: bool,
}

impl<'a> Sorter<'a> {
    /// Input is capped far below 4 GiB, so offsets and indices fit in u32.
    pub(crate) fn new(text: &'a str, recursive: bool) -> Self {
        Self {
            text,
            entries: Vec::new(),
            open: Vec::new(),
            recursive,
        }
    }

    /// Buffers an event whose tokens are slices of the sorter's text.
    pub(crate) fn push(&mut self, event: Event<'a>) {
        let token = |raw: &str| {
            let offset = raw.as_ptr() as usize - self.text.as_ptr() as usize;
            (offset as u32, raw.len() as u32)
        };
        let (kind, (a, b)) = match event {
            Event::Open(container) => {
                self.open.push(self.entries.len());
                (Kind::Open(container), (0, 0))
            }
            Event::Close(container) => {
                let end = self.entries.len() + 1;
                if let Some(start) = self.open.pop() {
                    self.entries[start].a = end as u32;
                }
                (Kind::Close(container), (0, 0))
            }
            Event::Key(raw) => (Kind::Key, token(raw)),
            Event::Value(raw) => (Kind::Value, token(raw)),
        };
        self.entries.push(Entry { kind, a, b });
    }

    pub(crate) fn replay(self, sink: &mut dyn Sink, ctx: &Context) -> Result<(), TransformError> {
        let mut replay = Replay {
            sorter: &self,
            ctx,
            count: 0,
        };
        let mut i = 0;
        while i < self.entries.len() {
            i = replay.value(i, 0, sink)?;
        }
        Ok(())
    }

    fn token(&self, entry: Entry) -> &'a str {
        &self.text[entry.a as usize..(entry.a + entry.b) as usize]
    }

    /// The index just past the value starting at `i`.
    fn end(&self, i: usize) -> usize {
        match self.entries[i].kind {
            Kind::Open(_) => self.entries[i].a as usize,
            _ => i + 1,
        }
    }
}

struct Replay<'r, 'a> {
    sorter: &'r Sorter<'a>,
    ctx: &'r Context<'r>,
    count: usize,
}

impl Replay<'_, '_> {
    fn emit(&mut self, i: usize, sink: &mut dyn Sink) -> Result<(), TransformError> {
        self.ctx.tick(self.count)?;
        self.count += 1;
        let entry = self.sorter.entries[i];
        sink.event(match entry.kind {
            Kind::Open(container) => Event::Open(container),
            Kind::Close(container) => Event::Close(container),
            Kind::Key => Event::Key(self.sorter.token(entry)),
            Kind::Value => Event::Value(self.sorter.token(entry)),
        })
    }

    /// Replays the value starting at `i`; returns the index after it.
    fn value(
        &mut self,
        i: usize,
        depth: usize,
        sink: &mut dyn Sink,
    ) -> Result<usize, TransformError> {
        let end = self.sorter.end(i);
        self.emit(i, sink)?;
        match self.sorter.entries[i].kind {
            Kind::Open(Container::Array) => {
                let mut j = i + 1;
                while j < end - 1 {
                    j = self.value(j, depth + 1, sink)?;
                }
            }
            Kind::Open(Container::Object) => {
                let mut members: Vec<(Cow<'_, str>, usize)> = Vec::new();
                let mut j = i + 1;
                while j < end - 1 {
                    members.push((decode_key(self.sorter.token(self.sorter.entries[j])), j));
                    j = self.sorter.end(j + 1);
                }
                if depth == 0 || self.sorter.recursive {
                    members.sort_by(|a, b| a.0.cmp(&b.0));
                }
                for (_, key) in members {
                    self.emit(key, sink)?;
                    self.value(key + 1, depth + 1, sink)?;
                }
            }
            _ => return Ok(end),
        }
        self.emit(end - 1, sink)?;
        Ok(end)
    }
}
