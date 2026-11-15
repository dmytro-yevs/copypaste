//! Commands: the surfaces' entry points and every action the views carry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use copypaste_module_sdk::{
    Delivery, HostError, ModuleInvocation, ModuleOutput, ModuleView, NoticeTone, ResultItem,
    MAX_LABEL_CHARS, MAX_RESULTS,
};
use copypaste_template::Severity;
use serde_json::{Map, Value};

use crate::abbreviation::{self, AbbreviationError};
use crate::bundle::TransferError;
use crate::export::{self, ExportScope};
use crate::host::{self, Host};
use crate::import::{
    self, AbbreviationConflictPolicy, CollectionNamePolicy, ImportPlan, ImportPolicies,
    SameIdPolicy,
};
use crate::library::{CollectionDraft, Draft, Library, LibraryError, Scope};
use crate::model;
use crate::ranking::SearchTier;
use crate::render::{self, Clipboard, Templates};
use crate::strings::Strings;
use crate::views::{
    self, collection_option, Browse, Editor, EditorValues, PREF_RECORD_PASTED, PREF_REQUIRE_AUTH,
    UNFILED,
};

const MSG_UNKNOWN: &str = "This command is not available.";
const MSG_REQUIRED: &str = "Fill in the required fields before using this snippet.";
const MSG_PLAN_GONE: &str = "This import preview has ended. Preview the file again.";
const MSG_MISMATCH: &str = "The passphrases do not match.";
const MSG_RENDER: &str = "The snippet could not be expanded.";
/// How long an unused import preview is kept.
const PLAN_TTL: Duration = Duration::from_secs(10 * 60);
/// The file name the host suggests for an export.
const EXPORT_NAME: &str = "copypaste-snippets.copypaste-snippets";
const EXPORT_DIR: &str = "exports";

pub(crate) enum Failure {
    Host(HostError),
    Message(String),
}

impl Failure {
    pub(crate) fn message(self) -> String {
        match self {
            Self::Host(error) => error.message,
            Self::Message(message) => message,
        }
    }
}

impl From<LibraryError> for Failure {
    fn from(error: LibraryError) -> Self {
        match error {
            LibraryError::Host(error) => Self::Host(error),
            other => Self::Message(other.message()),
        }
    }
}

impl From<HostError> for Failure {
    fn from(error: HostError) -> Self {
        Self::Host(error)
    }
}

impl From<TransferError> for Failure {
    fn from(error: TransferError) -> Self {
        match error {
            TransferError::Library(error) => error.into(),
            other => Self::Message(other.message()),
        }
    }
}

type Outcome = Result<ModuleOutput, Failure>;

struct PendingImport {
    plan: ImportPlan,
    at: Instant,
}

/// The module's state across invocations.
pub(crate) struct State {
    library: Library,
    templates: Templates,
    data_dir: PathBuf,
    pending: Option<PendingImport>,
    migrated: bool,
}

/// The synced preferences the host passes with each invocation.
#[derive(Debug, Clone, Copy, Default)]
struct Preferences {
    record_pasted: bool,
    require_auth: bool,
}

impl Preferences {
    fn from(values: &BTreeMap<String, Value>) -> Self {
        let flag = |id| values.get(id).and_then(Value::as_bool).unwrap_or(false);
        Self {
            record_pasted: flag(PREF_RECORD_PASTED),
            require_auth: flag(PREF_REQUIRE_AUTH),
        }
    }
}

/// One invocation's arguments.
struct Args<'a>(&'a BTreeMap<String, Value>);

impl Args<'_> {
    fn text(&self, key: &str) -> &str {
        self.0.get(key).and_then(Value::as_str).unwrap_or_default()
    }

    fn flag(&self, key: &str) -> bool {
        match self.0.get(key) {
            Some(Value::Bool(value)) => *value,
            Some(Value::String(value)) => value == "true",
            _ => false,
        }
    }

    fn ids(&self) -> Vec<String> {
        self.0
            .get("ids")
            .and_then(Value::as_array)
            .map(|ids| {
                ids.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn values(&self) -> Map<String, Value> {
        self.0
            .get("values")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    }

    fn browse(&self) -> Browse {
        let value = self.text("value");
        Browse {
            scope: Scope::parse(if value.is_empty() {
                self.text("scope")
            } else {
                value
            }),
            query: self.text("query").to_owned(),
            page: self.text("page").parse().unwrap_or(1),
        }
    }
}

fn text_value<'a>(values: &'a Map<String, Value>, key: &str) -> &'a str {
    values.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn bool_value(values: &Map<String, Value>, key: &str) -> bool {
    values.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn choice_value<'a>(values: &'a Map<String, Value>, key: &str) -> &'a str {
    match values.get(key) {
        Some(Value::Array(chosen)) => chosen.first().and_then(Value::as_str).unwrap_or_default(),
        Some(Value::String(chosen)) => chosen,
        _ => "",
    }
}

fn some(text: &str) -> Option<&str> {
    Some(text).filter(|text| !text.is_empty())
}

fn message(text: impl Into<String>) -> Outcome {
    Ok(ModuleOutput::Message {
        message: text.into(),
    })
}

fn view(view: ModuleView) -> Outcome {
    Ok(ModuleOutput::View { view })
}

/// Clipboard text uses the platform's line endings: CRLF on Windows.
fn clipboard_line_endings(text: String) -> String {
    if cfg!(windows) && text.contains('\n') {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text
    }
}

impl State {
    pub(crate) fn new(host: Arc<dyn Host>, data_dir: PathBuf) -> Self {
        Self {
            library: Library::new(host),
            templates: Templates::default(),
            data_dir,
            pending: None,
            migrated: false,
        }
    }

    pub(crate) fn run(&mut self, invocation: &ModuleInvocation) -> Outcome {
        let preferences = Preferences::from(&invocation.preferences);
        let locale = host::context(self.library.host())
            .map(|context| context.locale)
            .unwrap_or_else(|| "en".to_owned());
        let strings = Strings::new(&locale);
        self.library.drain_conflicts()?;
        self.library.ensure()?;
        if !self.migrated {
            crate::migration::run(&mut self.library)?;
            self.migrated = true;
            self.library.ensure()?;
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.at.elapsed() > PLAN_TTL)
        {
            self.pending = None;
        }
        let args = Args(&invocation.arguments);
        let context = Context {
            strings,
            locale: &locale,
            preferences,
        };
        match invocation.command.as_str() {
            "library" => self.browse(&context, Browse::default()),
            "browse" => self.browse(&context, args.browse()),
            "new" => self.new_snippet(&context, None, String::new()),
            "save-item" => self.new_snippet(&context, None, args.text("text").to_owned()),
            "edit" => match some(args.text("id")) {
                Some(id) => self.edit(&context, id),
                None => self.new_snippet(&context, some(args.text("collection")), String::new()),
            },
            "save" => self.save(&context, &args),
            "preview" => self.preview(&context, &args),
            "delete" => {
                let deleted = self.library.delete(&args.ids())?;
                message(context.strings.plural("snippetsDeletedTitle", deleted, &[]))
            }
            "restore" => {
                self.library.restore(&args.ids())?;
                message(context.strings.get("commonDone"))
            }
            "purge" => {
                self.library.purge(&args.ids())?;
                message(context.strings.get("commonDone"))
            }
            "pin" => {
                self.library
                    .set_pinned(args.text("id"), args.flag("pinned"))?;
                self.browse(&context, args.browse())
            }
            "reorder" => {
                let browse = args.browse();
                let before = some(args.text("before"));
                if browse.scope == Scope::Pinned {
                    self.library.reorder_pin(args.text("id"), before)?;
                } else {
                    self.library.reorder(args.text("id"), before)?;
                }
                self.browse(&context, browse)
            }
            "move-to" => view(views::move_form(
                &context.strings,
                &self.library.collections(),
                &args.ids(),
            )),
            "move" => {
                let values = args.values();
                let target = self.collection_from_option(choice_value(&values, "collection"));
                self.library.move_to(&args.ids(), target.as_deref())?;
                message(context.strings.get("commonDone"))
            }
            "collection-edit" => self.collection_edit(&context, some(args.text("id"))),
            "collection-save" => self.collection_save(&context, &args),
            "collection-delete-ask" => {
                let id = args.text("id");
                let name = self.library.live_collection(id)?.name.clone();
                view(views::collection_delete(&context.strings, id, &name))
            }
            "collection-delete" => {
                self.library
                    .delete_collection(args.text("id"), args.flag("with_snippets"))?;
                message(context.strings.get("commonDone"))
            }
            "collection-merge-ask" => {
                let id = args.text("id");
                let name = self.library.live_collection(id)?.name.clone();
                view(views::collection_merge(
                    &context.strings,
                    &self.library.collections(),
                    id,
                    &name,
                ))
            }
            "collection-merge" => {
                let values = args.values();
                let into = self
                    .collection_from_option(choice_value(&values, "into"))
                    .ok_or(LibraryError::InvalidCollection)?;
                self.library.merge_collections(args.text("id"), &into)?;
                message(context.strings.get("commonDone"))
            }
            "import" => view(views::import_form(
                &context.strings,
                Map::new(),
                BTreeMap::new(),
            )),
            "import-preview" => self.import_preview(&context, &args),
            "import-commit" => self.import_commit(&context, &args),
            "export" => view(views::export_form(
                &context.strings,
                args.text("scope"),
                &args.ids(),
                Map::new(),
                BTreeMap::new(),
            )),
            "export-run" => self.export(&context, &args),
            "search" => self.search(&context, args.text("query")),
            "use" => self.activate(&context, args.text("id")),
            "fill" => {
                let values = args.values();
                self.deliver(&context, args.text("id"), Some(&values))
            }
            "fill-preview" => self.fill_preview(&context, &args),
            "serve" => Ok(ModuleOutput::Data {
                data: crate::service::serve(
                    &mut self.library,
                    &self.templates,
                    &locale,
                    args.text("request"),
                ),
            }),
            _ => Err(Failure::Message(MSG_UNKNOWN.into())),
        }
    }

    fn browse(&mut self, context: &Context<'_>, browse: Browse) -> Outcome {
        view(views::library(
            &mut self.library,
            &context.strings,
            browse,
            context.preferences.require_auth,
        ))
    }

    fn collection_from_option(&self, option: &str) -> Option<String> {
        if option.is_empty() || option == UNFILED {
            return None;
        }
        self.library
            .collections()
            .into_iter()
            .find(|view| collection_option(&view.id) == option)
            .map(|view| view.id)
    }

    fn new_snippet(
        &mut self,
        context: &Context<'_>,
        collection: Option<&str>,
        body: String,
    ) -> Outcome {
        let values = EditorValues {
            collection: collection
                .filter(|id| self.library.live_collection(id).is_ok())
                .map(str::to_owned),
            body,
            ..EditorValues::default()
        };
        view(views::editor(
            &context.strings,
            &self.library.collections(),
            Editor {
                id: None,
                base: None,
                values: &values,
                errors: BTreeMap::new(),
                notices: Vec::new(),
                deleted: false,
            },
        ))
    }

    fn edit(&mut self, context: &Context<'_>, id: &str) -> Outcome {
        let meta = self.library.meta(id).ok_or(LibraryError::NotFound)?.clone();
        let body = self.library.body(id)?.unwrap_or_default();
        let values = EditorValues {
            title: meta.title.clone(),
            abbreviation: meta.abbreviation.clone().unwrap_or_default(),
            collection: self.library.filed_in(&meta).map(str::to_owned),
            body: body.content().to_owned(),
            template: meta.template,
            pinned: meta.pinned,
            concealed: meta.concealed,
        };
        let mut notices = Vec::new();
        if self.library.duplicates().contains(id) {
            notices.push((
                context
                    .strings
                    .get("snippetsDuplicateAbbreviation")
                    .to_owned(),
                NoticeTone::Warning,
            ));
        }
        if body.rich_html.is_some() && !meta.template {
            notices.push((
                context.strings.get("snippetsFormattingNotice").to_owned(),
                NoticeTone::Info,
            ));
        }
        let mut errors = BTreeMap::new();
        if let Some(expansion) = render::stored(
            id,
            &self.library,
            &body,
            &BTreeMap::new(),
            Clipboard::None,
            context.locale,
        ) {
            let lints = expansion.lint(&self.library, &self.templates);
            if let Some(first) = render::lint_messages(&context.strings, &lints, Severity::Error)
                .into_iter()
                .next()
            {
                errors.insert("body".to_owned(), views::short(&first, 1024));
            }
        }
        view(views::editor(
            &context.strings,
            &self.library.collections(),
            Editor {
                id: Some(id),
                base: Some(&body.revision),
                values: &values,
                errors,
                notices,
                deleted: meta.deleted_at_ms.is_some(),
            },
        ))
    }

    fn editor_values(&self, values: &Map<String, Value>) -> EditorValues {
        EditorValues {
            title: text_value(values, "title").to_owned(),
            abbreviation: text_value(values, "abbreviation").to_owned(),
            collection: self.collection_from_option(choice_value(values, "collection")),
            body: text_value(values, "body").to_owned(),
            template: bool_value(values, "template"),
            pinned: bool_value(values, "pinned"),
            concealed: bool_value(values, "concealed"),
        }
    }

    fn save(&mut self, context: &Context<'_>, args: &Args<'_>) -> Outcome {
        let strings = &context.strings;
        let values = args.values();
        let editor = self.editor_values(&values);
        let id = some(args.text("id"));
        let base = some(args.text("base"));
        let wanted = some(editor.abbreviation.trim());
        let previous = id
            .and_then(|id| self.library.meta(id))
            .and_then(|meta| meta.abbreviation.clone());
        let renamed = match (&previous, wanted) {
            (Some(previous), Some(wanted)) => {
                abbreviation::normalize(previous) != abbreviation::normalize(wanted)
            }
            _ => false,
        };
        let draft = Draft {
            id: id.map(str::to_owned),
            collection_id: editor.collection.clone(),
            title: editor.title.clone(),
            abbreviation: wanted.map(str::to_owned),
            body_text: editor.body.clone(),
            body_rich: None,
            template: editor.template,
            pinned: editor.pinned,
            concealed: editor.concealed,
            base_revision: base.map(str::to_owned),
        };
        let mut result = Ok(String::new());
        if let (true, Some(id), Some(previous), Some(wanted)) = (renamed, id, &previous, wanted) {
            match args.text("references") {
                "" if !self.library.references(id)?.is_empty() => {
                    return view(views::references_prompt(
                        strings,
                        id,
                        base.unwrap_or_default(),
                        &values,
                        previous,
                        wanted,
                    ));
                }
                "update" => {
                    result = self
                        .library
                        .rename_abbreviation(id, Some(wanted), true)
                        .map(|_| String::new());
                }
                _ => {}
            }
        }
        if result.is_ok() {
            result = self.library.save(&draft, None);
        }
        let failure = match result {
            Ok(_) => return message(strings.get("commonDone")),
            Err(error) => error,
        };
        let mut errors = BTreeMap::new();
        let mut notices = Vec::new();
        match failure {
            LibraryError::AbbreviationTaken { title, .. } => {
                let title = views::untitled(strings, &title).to_owned();
                errors.insert(
                    "abbreviation".to_owned(),
                    strings.fill("snippetsAbbreviationTaken", &[("title", &title)]),
                );
            }
            LibraryError::InvalidAbbreviation(error) => {
                let text = match error {
                    AbbreviationError::TooLong => strings.fill(
                        "snippetsAbbreviationTooLong",
                        &[("max", &abbreviation::MAX_GRAPHEMES.to_string())],
                    ),
                    _ => strings.get("snippetsAbbreviationWhitespace").to_owned(),
                };
                errors.insert("abbreviation".to_owned(), text);
            }
            LibraryError::EmptyBody => {
                errors.insert(
                    "body".to_owned(),
                    strings.get("snippetsFieldRequired").to_owned(),
                );
            }
            LibraryError::TooLarge => {
                errors.insert(
                    "body".to_owned(),
                    strings.get("snippetsSaveBlockTooLarge").to_owned(),
                );
            }
            LibraryError::InvalidTitle => {
                errors.insert("title".to_owned(), LibraryError::InvalidTitle.message());
            }
            LibraryError::InvalidCollection => {
                errors.insert(
                    "collection".to_owned(),
                    LibraryError::InvalidCollection.message(),
                );
            }
            LibraryError::Host(error) => return Err(Failure::Host(error)),
            other => notices.push((other.message(), NoticeTone::Error)),
        }
        view(views::editor(
            strings,
            &self.library.collections(),
            Editor {
                id,
                base,
                values: &editor,
                errors,
                notices,
                deleted: id
                    .and_then(|id| self.library.meta(id))
                    .is_some_and(|meta| meta.deleted_at_ms.is_some()),
            },
        ))
    }

    /// The editor's preview: the rendered text, or what is wrong with it.
    fn preview(&mut self, context: &Context<'_>, args: &Args<'_>) -> Outcome {
        let values = args.values();
        let editor = self.editor_values(&values);
        if editor.body.trim().is_empty() {
            return message(context.strings.get("snippetsPreviewEmpty"));
        }
        let id = some(args.text("id"));
        let no_values = BTreeMap::new();
        let expansion = render::Expansion {
            body: &editor.body,
            revision: None,
            template: editor.template,
            origin: id.map(|id| copypaste_template::Origin {
                id: id.to_owned(),
                abbreviation: some(editor.abbreviation.trim()).map(str::to_owned),
            }),
            concealed: editor.concealed,
            values: &no_values,
            clipboard: Clipboard::Host(self.library.host()),
            locale: context.locale,
        };
        let lints = expansion.lint(&self.library, &self.templates);
        let problems = render::lint_messages(&context.strings, &lints, Severity::Warning);
        if editor.template && lints.iter().any(|lint| lint.severity == Severity::Error) {
            return message(views::short(&problems.join("\n"), 4096));
        }
        let rendered = expansion
            .render(&self.library, &self.templates)
            .map_err(|_| Failure::Message(MSG_RENDER.into()))?;
        if rendered.text.is_empty() {
            return message(context.strings.get("snippetsPreviewEmpty"));
        }
        Ok(ModuleOutput::Text {
            text: rendered.text,
        })
    }

    fn collection_edit(&mut self, context: &Context<'_>, id: Option<&str>) -> Outcome {
        let (name, accent) = match id {
            Some(id) => {
                let collection = self.library.live_collection(id)?;
                (collection.name.clone(), collection.accent.clone())
            }
            None => (String::new(), "neutral".to_owned()),
        };
        view(views::collection_editor(
            &context.strings,
            id,
            &name,
            &accent,
            BTreeMap::new(),
        ))
    }

    fn collection_save(&mut self, context: &Context<'_>, args: &Args<'_>) -> Outcome {
        let strings = &context.strings;
        let values = args.values();
        let id = some(args.text("id"));
        let name = text_value(&values, "name");
        let accent = some(choice_value(&values, "accent")).unwrap_or("neutral");
        let icon = id
            .and_then(|id| self.library.live_collection(id).ok())
            .map(|collection| collection.icon.clone())
            .unwrap_or_default();
        let draft = CollectionDraft {
            id: id.map(str::to_owned),
            name: name.to_owned(),
            icon,
            accent: accent.to_owned(),
        };
        let error = match self.library.save_collection(&draft, None) {
            Ok(_) => return message(strings.get("commonDone")),
            Err(LibraryError::CollectionNameTaken { .. }) => {
                strings.get("snippetsCollectionNameTaken").to_owned()
            }
            Err(LibraryError::InvalidCollection)
                if name.trim().chars().count() > crate::library::MAX_COLLECTION_NAME_CHARS =>
            {
                strings.fill(
                    "snippetsCollectionNameTooLong",
                    &[(
                        "max",
                        &crate::library::MAX_COLLECTION_NAME_CHARS.to_string(),
                    )],
                )
            }
            Err(LibraryError::InvalidCollection) => LibraryError::InvalidCollection.message(),
            Err(error) => return Err(error.into()),
        };
        view(views::collection_editor(
            strings,
            id,
            name,
            accent,
            BTreeMap::from([("name".to_owned(), error)]),
        ))
    }

    fn import_preview(&mut self, context: &Context<'_>, args: &Args<'_>) -> Outcome {
        let values = args.values();
        let path = PathBuf::from(text_value(&values, "file"));
        let passphrase = some(text_value(&values, "passphrase"));
        let convert = values
            .get("convert")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        self.pending = None;
        let parsed = (|| {
            if !path.is_absolute() {
                return Err(TransferError::Io);
            }
            let format = import::detect(&path)?;
            let parsed = import::read(&path, passphrase, format, convert)?;
            import::plan(&self.library, parsed)
        })();
        match parsed {
            Ok(plan) => {
                let preview = views::import_preview(&context.strings, &plan);
                self.pending = Some(PendingImport {
                    plan,
                    at: Instant::now(),
                });
                view(preview)
            }
            Err(TransferError::Library(error)) => Err(error.into()),
            Err(error) => {
                let field = match error {
                    TransferError::PassphraseRequired
                    | TransferError::WrongPassphrase
                    | TransferError::WeakPassphrase => "passphrase",
                    _ => "file",
                };
                view(views::import_form(
                    &context.strings,
                    values.clone(),
                    BTreeMap::from([(field.to_owned(), error.message())]),
                ))
            }
        }
    }

    fn import_commit(&mut self, context: &Context<'_>, args: &Args<'_>) -> Outcome {
        let token = args.text("token");
        let pending = self
            .pending
            .take_if(|pending| pending.plan.token() == token)
            .ok_or_else(|| Failure::Message(MSG_PLAN_GONE.into()))?;
        let values = args.values();
        let policies = ImportPolicies {
            same_id_different: match choice_value(&values, "same_id") {
                "replace" => SameIdPolicy::Replace,
                "skip" => SameIdPolicy::Skip,
                _ => SameIdPolicy::KeepBoth,
            },
            abbreviation_conflict: match choice_value(&values, "abbreviation") {
                "rename-with-suffix" => AbbreviationConflictPolicy::RenameWithSuffix,
                "skip" => AbbreviationConflictPolicy::Skip,
                _ => AbbreviationConflictPolicy::ImportWithout,
            },
            collection_name: match choice_value(&values, "collections") {
                "create-new" => CollectionNamePolicy::CreateNew,
                _ => CollectionNamePolicy::Merge,
            },
        };
        let report = import::commit(&mut self.library, pending.plan, policies)?;
        message(context.strings.fill(
            "snippetsImportReport",
            &[
                ("inserted", &report.inserted.to_string()),
                ("updated", &report.updated.to_string()),
                ("skipped", &report.skipped.to_string()),
                (
                    "flagged",
                    &(report.renamed_abbreviations + report.without_abbreviation).to_string(),
                ),
                (
                    "unrecognized",
                    &report.unrecognized_placeholders.to_string(),
                ),
            ],
        ))
    }

    fn export(&mut self, context: &Context<'_>, args: &Args<'_>) -> Outcome {
        let values = args.values();
        let ids = args.ids();
        let scope_id = args.text("scope");
        let passphrase = text_value(&values, "passphrase");
        let confirm = text_value(&values, "passphrase_confirm");
        let mut errors = BTreeMap::new();
        if !passphrase.is_empty()
            && passphrase.chars().count() < crate::bundle::MIN_PASSPHRASE_CHARS
        {
            errors.insert(
                "passphrase".to_owned(),
                context
                    .strings
                    .fill("snippetsPassphraseHint", &[("min", "8")]),
            );
        } else if passphrase != confirm {
            errors.insert("passphrase_confirm".to_owned(), MSG_MISMATCH.to_owned());
        }
        if !errors.is_empty() {
            return view(views::export_form(
                &context.strings,
                scope_id,
                &ids,
                values,
                errors,
            ));
        }
        let scope = if !ids.is_empty() {
            ExportScope::Snippets(ids)
        } else {
            match Scope::parse(scope_id) {
                Scope::Collection(id) => ExportScope::Collection(id),
                _ => ExportScope::All,
            }
        };
        let relative = format!("{EXPORT_DIR}/{}.json", model::new_id());
        let path = self.data_dir.join(Path::new(&relative));
        export::export(
            &self.library,
            &path,
            &scope,
            some(passphrase),
            bool_value(&values, "include_concealed"),
        )?;
        Ok(ModuleOutput::SaveFile {
            path: relative,
            name: EXPORT_NAME.to_owned(),
        })
    }

    /// Quick Paste: ranked matches, or the tab's list for an empty query.
    fn search(&mut self, context: &Context<'_>, query: &str) -> Outcome {
        let hits: Vec<(String, Option<SearchTier>)> = if query.trim().is_empty() {
            self.library
                .recent(MAX_RESULTS)
                .into_iter()
                .map(|id| (id, None))
                .collect()
        } else {
            self.library
                .quick_search(query, MAX_RESULTS)?
                .into_iter()
                .map(|hit| (hit.id, Some(hit.tier)))
                .collect()
        };
        let exact = hits
            .iter()
            .filter(|(_, tier)| *tier == Some(SearchTier::ExactAbbreviation))
            .count();
        let duplicates = self.library.duplicates();
        let strings = &context.strings;
        let items = hits
            .into_iter()
            .filter_map(|(id, tier)| {
                let meta = self.library.live(&id).ok()?;
                let mut badges = Vec::new();
                if let Some(abbreviation) = &meta.abbreviation {
                    badges.push(views::short(abbreviation, 32));
                }
                if meta.template {
                    badges.push(views::short(strings.get("snippetsTemplateBadge"), 32));
                }
                if duplicates.contains(&id) {
                    badges.push(views::short(strings.get("snippetsDuplicateBadge"), 32));
                }
                Some(ResultItem {
                    title: views::short(views::untitled(strings, &meta.title), 256),
                    preview: if meta.concealed {
                        strings.get("snippetsConcealedPreview").to_owned()
                    } else {
                        views::short(&meta.preview, 1024)
                    },
                    badges,
                    secret: meta.concealed,
                    preselect: exact == 1 && tier == Some(SearchTier::ExactAbbreviation),
                    actions: Vec::new(),
                    id,
                })
            })
            .collect();
        Ok(ModuleOutput::Results { items })
    }

    /// Quick Paste activation: the text, or the template's fill-in form.
    fn activate(&mut self, context: &Context<'_>, id: &str) -> Outcome {
        let meta = self.library.live(id)?.clone();
        let body = self.library.body(id)?.ok_or(LibraryError::NotFound)?;
        let no_values = BTreeMap::new();
        let fields = render::stored(
            id,
            &self.library,
            &body,
            &no_values,
            Clipboard::None,
            context.locale,
        )
        .map(|expansion| expansion.fields(&self.library, &self.templates))
        .unwrap_or_default();
        if fields.is_empty() {
            return self.deliver(context, id, None);
        }
        view(views::fill(
            &context.strings,
            id,
            &meta.title,
            &fields,
            !meta.concealed,
        ))
    }

    /// Renders with the submitted values (or the fields' defaults) and hands
    /// the text to the host. Concealed output is secret and never recorded.
    fn deliver(
        &mut self,
        context: &Context<'_>,
        id: &str,
        submitted: Option<&Map<String, Value>>,
    ) -> Outcome {
        let rendered = self.render_for_use(context, id, submitted)?;
        if !rendered.missing_required.is_empty() {
            return Err(Failure::Message(MSG_REQUIRED.into()));
        }
        let title = self.library.live(id)?.title.clone();
        let label: String = views::untitled(&context.strings, &title)
            .chars()
            .take(MAX_LABEL_CHARS)
            .collect();
        let secret = rendered.secret;
        let cursor_graphemes_from_end = rendered.cursor_graphemes_from_end;
        // Usage only orders results; a failed write does not block the use.
        let _ = self.library.record_use(id, model::now_ms());
        Ok(ModuleOutput::Deliver {
            delivery: Delivery {
                text: clipboard_line_endings(rendered.text),
                secret,
                require_auth: secret && context.preferences.require_auth,
                record_history: !secret && context.preferences.record_pasted,
                label,
                cursor_graphemes_from_end,
            },
        })
    }

    fn render_for_use(
        &self,
        context: &Context<'_>,
        id: &str,
        submitted: Option<&Map<String, Value>>,
    ) -> Result<render::Rendering, Failure> {
        self.library.live(id)?;
        let body = self.library.body(id)?.ok_or(LibraryError::NotFound)?;
        let no_values = BTreeMap::new();
        let values = match submitted {
            Some(submitted) => {
                let fields = render::stored(
                    id,
                    &self.library,
                    &body,
                    &no_values,
                    Clipboard::None,
                    context.locale,
                )
                .map(|expansion| expansion.fields(&self.library, &self.templates))
                .unwrap_or_default();
                views::fill_values(&fields, submitted)
            }
            None => BTreeMap::new(),
        };
        let expansion = render::stored(
            id,
            &self.library,
            &body,
            &values,
            Clipboard::Host(self.library.host()),
            context.locale,
        )
        .ok_or(LibraryError::NotFound)?;
        expansion
            .render(&self.library, &self.templates)
            .map_err(|_| Failure::Message(MSG_RENDER.into()))
    }

    fn fill_preview(&mut self, context: &Context<'_>, args: &Args<'_>) -> Outcome {
        let id = args.text("id");
        if self.library.live(id)?.concealed {
            return message(context.strings.get("snippetsConcealedBody"));
        }
        let values = args.values();
        let rendered = self.render_for_use(context, id, Some(&values))?;
        if rendered.secret {
            return message(context.strings.get("snippetsConcealedBody"));
        }
        if rendered.text.is_empty() {
            return message(context.strings.get("snippetsPreviewEmpty"));
        }
        Ok(ModuleOutput::Text {
            text: rendered.text,
        })
    }
}

struct Context<'a> {
    strings: Strings,
    locale: &'a str,
    preferences: Preferences,
}
