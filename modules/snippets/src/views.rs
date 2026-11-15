//! Host-rendered views: the library section, editors, collection forms,
//! import and export, and the Quick Paste fill-in form.

use std::collections::BTreeMap;

use copypaste_module_sdk::{
    FilterOption, ModuleChoice, ModuleField, ModuleFieldValue, ModuleIcon, ModuleView, NoticeTone,
    ViewAction, ViewForm, ViewItem, ViewSection, ViewTarget, MAX_FILTER_OPTIONS,
};
use copypaste_template::{FieldKind, TemplateField};
use serde_json::{json, Map, Value};

use crate::import::{ImportCounts, ImportPlan, ImportRowOutcome};
use crate::library::{
    CollectionView, Library, Scope, MAX_BODY_BYTES, MAX_COLLECTION_NAME_CHARS, SOFT_SNIPPET_LIMIT,
};
use crate::strings::Strings;

/// Rows of one page; a view shows at most two pages, within the host's
/// item limit.
pub(crate) const PAGE: usize = 100;
/// Collections offered in one choice field, beside Unfiled.
const MAX_COLLECTION_CHOICES: usize = copypaste_module_sdk::MAX_CHOICE_OPTIONS - 1;
const MAX_BADGE_CHARS: usize = 32;
const MAX_LISTED_ROWS: usize = 50;
pub(crate) const ACCENTS: [&str; 5] = ["neutral", "blue", "green", "amber", "red"];

/// Preferences of this module, by id.
pub(crate) const PREF_RECORD_PASTED: &str = "record-pasted";
pub(crate) const PREF_REQUIRE_AUTH: &str = "require-auth";

pub(crate) fn action(title: &str, command: &str) -> ViewAction {
    ViewAction::new(title, command)
}

pub(crate) trait ActionExt {
    fn arg(self, key: &str, value: impl Into<Value>) -> Self;
    fn drawer(self) -> Self;
    fn destructive(self) -> Self;
    fn auth(self, on: bool) -> Self;
}

impl ActionExt for ViewAction {
    fn arg(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.arguments.insert(key.to_owned(), value.into());
        self
    }

    fn drawer(mut self) -> Self {
        self.target = ViewTarget::Drawer;
        self
    }

    fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    fn auth(mut self, on: bool) -> Self {
        self.require_auth = on;
        self
    }
}

fn field(id: &str, title: &str, value: ModuleFieldValue) -> ModuleField {
    ModuleField {
        id: id.to_owned(),
        title: short(title, 160),
        synced: false,
        value,
    }
}

pub(crate) fn text_field(id: &str, title: &str, required: bool, max_bytes: u32) -> ModuleField {
    field(
        id,
        title,
        ModuleFieldValue::Text {
            default: String::new(),
            secret: false,
            required,
            multiline: false,
            template: false,
            max_bytes: Some(max_bytes),
        },
    )
}

fn secret_field(id: &str, title: &str) -> ModuleField {
    field(
        id,
        title,
        ModuleFieldValue::Text {
            default: String::new(),
            secret: true,
            required: false,
            multiline: false,
            template: false,
            max_bytes: Some(1024),
        },
    )
}

fn bool_field(id: &str, title: &str, default: bool) -> ModuleField {
    field(id, title, ModuleFieldValue::Boolean { default })
}

fn choice_field(id: &str, title: &str, options: Vec<(String, String)>) -> ModuleField {
    field(
        id,
        title,
        ModuleFieldValue::Choices {
            options: options
                .into_iter()
                .map(|(id, title)| ModuleChoice {
                    id,
                    title: short(&title, 160),
                })
                .collect(),
            default: Vec::new(),
            required: true,
        },
    )
}

/// At most `max` characters; never empty when `text` is not.
pub(crate) fn short(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn badge(text: &str) -> String {
    short(text, MAX_BADGE_CHARS)
}

/// A choice id for a collection: `c-<id>` when that is a valid id,
/// otherwise derived from it.
pub(crate) fn collection_option(id: &str) -> String {
    let candidate = format!("c-{id}");
    if copypaste_module_sdk::valid_id(&candidate) {
        return candidate;
    }
    use sha2::{Digest, Sha256};
    format!("c-{}", &hex::encode(Sha256::digest(id.as_bytes()))[..24])
}

pub(crate) const UNFILED: &str = "unfiled";

fn collection_choices(
    strings: &Strings,
    collections: &[CollectionView],
    except: Option<&str>,
) -> Vec<(String, String)> {
    collections
        .iter()
        .filter(|view| Some(view.id.as_str()) != except)
        .take(MAX_COLLECTION_CHOICES)
        .map(|view| (collection_option(&view.id), view.display_name.clone()))
        .chain(except.is_none().then(|| {
            (
                UNFILED.to_owned(),
                strings.get("snippetsUnfiled").to_owned(),
            )
        }))
        .collect()
}

/// The library section's state, carried by its actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Browse {
    pub(crate) scope: Scope,
    pub(crate) query: String,
    pub(crate) page: usize,
}

impl Default for Browse {
    fn default() -> Self {
        Self {
            scope: Scope::All,
            query: String::new(),
            page: 1,
        }
    }
}

impl Browse {
    fn with_state(&self, action: ViewAction) -> ViewAction {
        action
            .arg("scope", self.scope.id())
            .arg("query", self.query.clone())
            .arg("page", self.page.to_string())
    }
}

pub(crate) fn untitled<'a>(strings: &Strings, title: &'a str) -> &'a str {
    if title.trim().is_empty() {
        strings.get("snippetsUntitled")
    } else {
        title
    }
}

/// The ids a scope lists for `query`, in display order.
fn listed(library: &mut Library, browse: &Browse) -> Vec<String> {
    let query = browse.query.trim();
    if query.is_empty() {
        return library
            .page(&browse.scope)
            .into_iter()
            .map(|(id, _)| id.clone())
            .collect();
    }
    if browse.scope == Scope::RecentlyDeleted {
        let folded = query.to_lowercase();
        return library
            .page(&Scope::RecentlyDeleted)
            .into_iter()
            .filter(|(_, meta)| {
                meta.title.to_lowercase().contains(&folded)
                    || meta
                        .abbreviation
                        .as_deref()
                        .is_some_and(|abbreviation| abbreviation.to_lowercase().contains(&folded))
            })
            .map(|(id, _)| id.clone())
            .collect();
    }
    let hits = library
        .quick_search(query, copypaste_module_sdk::MAX_RESULTS)
        .unwrap_or_default();
    hits.into_iter()
        .map(|hit| hit.id)
        .filter(|id| {
            library.meta(id).is_some_and(|meta| match &browse.scope {
                Scope::All | Scope::RecentlyDeleted => true,
                Scope::Pinned => meta.pinned,
                Scope::Unfiled => library.filed_in(meta).is_none(),
                Scope::Collection(collection) => {
                    library.filed_in(meta) == Some(collection.as_str())
                }
            })
        })
        .collect()
}

/// The Settings section.
pub(crate) fn library(
    library: &mut Library,
    strings: &Strings,
    mut browse: Browse,
    require_auth: bool,
) -> ModuleView {
    let collections = library.collections();
    if let Scope::Collection(id) = &browse.scope {
        if !collections.iter().any(|view| &view.id == id) {
            browse.scope = Scope::All;
        }
    }
    let mut items = vec![ViewItem::Search {
        query: short(&browse.query, 1024),
        placeholder: strings.get("snippetsSearchPlaceholder").to_owned(),
        action: action(strings.get("snippetsSearchPlaceholder"), "browse")
            .arg("scope", browse.scope.id()),
    }];

    let count = |scope: &Scope| u32::try_from(library.page(scope).len()).unwrap_or(u32::MAX);
    let mut options = vec![
        FilterOption {
            id: Scope::All.id(),
            title: strings.get("snippetsScopeAll").to_owned(),
            count: Some(count(&Scope::All)),
            icon: Some(ModuleIcon::NotebookText),
        },
        FilterOption {
            id: Scope::Pinned.id(),
            title: strings.get("snippetsScopePinned").to_owned(),
            count: Some(count(&Scope::Pinned)),
            icon: Some(ModuleIcon::Bookmark),
        },
    ];
    let room = MAX_FILTER_OPTIONS - 4;
    options.extend(collections.iter().take(room).map(|view| FilterOption {
        id: Scope::Collection(view.id.clone()).id(),
        title: short(&view.display_name, 64),
        count: Some(u32::try_from(view.count).unwrap_or(u32::MAX)),
        icon: None,
    }));
    options.push(FilterOption {
        id: Scope::Unfiled.id(),
        title: strings.get("snippetsUnfiled").to_owned(),
        count: Some(count(&Scope::Unfiled)),
        icon: None,
    });
    options.push(FilterOption {
        id: Scope::RecentlyDeleted.id(),
        title: strings.get("snippetsScopeRecentlyDeleted").to_owned(),
        count: Some(count(&Scope::RecentlyDeleted)),
        icon: None,
    });
    if !options.iter().any(|option| option.id == browse.scope.id()) {
        browse.scope = Scope::All;
    }
    items.push(ViewItem::Filter {
        options,
        selected: browse.scope.id(),
        action: action(strings.get("snippetsShow"), "browse").arg("query", browse.query.clone()),
    });

    let collection = match &browse.scope {
        Scope::Collection(id) => Some(id.clone()),
        _ => None,
    };
    let mut toolbar = vec![
        action(strings.get("snippetsNewSnippet"), "edit")
            .arg("collection", collection.clone().unwrap_or_default())
            .drawer(),
        action(strings.get("snippetsNewCollection"), "collection-edit").drawer(),
        action(strings.get("snippetsImport"), "import").drawer(),
        action(strings.get("snippetsExport"), "export")
            .arg(
                "scope",
                collection
                    .as_ref()
                    .map_or_else(|| "all".to_owned(), |id| format!("c:{id}")),
            )
            .drawer(),
    ];
    if let Some(id) = &collection {
        toolbar.push(
            action(strings.get("snippetsEditCollection"), "collection-edit")
                .arg("id", id.clone())
                .drawer(),
        );
    }
    items.push(ViewItem::Toolbar { actions: toolbar });

    let total = library.snippet_total();
    if total > SOFT_SNIPPET_LIMIT {
        items.push(ViewItem::Notice {
            text: strings.fill("snippetsSoftLimit", &[("count", &total.to_string())]),
            tone: NoticeTone::Warning,
        });
    }
    let duplicates = library.duplicates();
    if !duplicates.is_empty() && browse.scope != Scope::RecentlyDeleted {
        items.push(ViewItem::Notice {
            text: strings.plural("snippetsAttention", duplicates.len(), &[]),
            tone: NoticeTone::Warning,
        });
    }

    let ids = listed(library, &browse);
    let page = browse.page.max(1);
    let start = page.saturating_sub(2) * PAGE;
    let end = (page * PAGE).min(ids.len());
    if ids.is_empty() {
        let text = if !browse.query.trim().is_empty() {
            strings.fill("snippetsNoMatch", &[("query", browse.query.trim())])
        } else if browse.scope == Scope::RecentlyDeleted {
            strings.get("snippetsRecentlyDeletedEmpty").to_owned()
        } else if total == 0 {
            format!(
                "{} {}",
                strings.get("snippetsEmptyTitle"),
                strings.get("snippetsEmptyMessage")
            )
        } else {
            strings.get("snippetsScopeEmpty").to_owned()
        };
        items.push(ViewItem::Notice {
            text: short(&text, 1024),
            tone: NoticeTone::Info,
        });
    }
    let deleted_scope = browse.scope == Scope::RecentlyDeleted;
    for id in ids.get(start.min(end)..end).unwrap_or_default() {
        let Some(meta) = library.meta(id) else {
            continue;
        };
        let mut badges = Vec::new();
        if let Some(abbreviation) = &meta.abbreviation {
            badges.push(badge(abbreviation));
        }
        if meta.template {
            badges.push(badge(strings.get("snippetsTemplateBadge")));
        }
        if duplicates.contains(id) {
            badges.push(badge(strings.get("snippetsDuplicateBadge")));
        }
        if id.starts_with("conflict-") {
            badges.push(badge(strings.get("snippetsConflictBadge")));
        }
        let actions = if deleted_scope {
            vec![
                action(strings.get("snippetsRestore"), "restore").arg("ids", json!([id])),
                action(strings.get("snippetsPurge"), "purge")
                    .arg("ids", json!([id]))
                    .destructive(),
            ]
        } else {
            vec![
                browse.with_state(
                    action(
                        strings.get(if meta.pinned {
                            "snippetsUnpin"
                        } else {
                            "snippetsPinnedSwitch"
                        }),
                        "pin",
                    )
                    .arg("id", id.clone())
                    .arg("pinned", !meta.pinned),
                ),
                action(strings.get("commonDelete"), "delete").arg("ids", json!([id])),
            ]
        };
        items.push(ViewItem::Row {
            title: short(untitled(strings, &meta.title), 256),
            subtitle: (!meta.concealed && !meta.preview.is_empty())
                .then(|| short(&meta.preview, 1024)),
            actions,
            id: Some(id.clone()),
            badges,
            secret: meta.concealed,
            icon: None,
            open: Some(
                action(strings.get("snippetsEditSnippet"), "edit")
                    .arg("id", id.clone())
                    .drawer()
                    .auth(meta.concealed && require_auth),
            ),
        });
    }

    let mut section = ViewSection::new(None, items);
    section.bulk_actions = if deleted_scope {
        vec![
            action(strings.get("snippetsRestore"), "restore"),
            action(strings.get("snippetsPurge"), "purge").destructive(),
        ]
    } else {
        vec![
            action(strings.get("snippetsMoveTo"), "move-to").drawer(),
            action(strings.get("snippetsExport"), "export").drawer(),
            action(strings.get("commonDelete"), "delete"),
        ]
    };
    let reorderable = browse.query.trim().is_empty()
        && matches!(
            browse.scope,
            Scope::Collection(_) | Scope::Unfiled | Scope::Pinned
        );
    if reorderable {
        section.reorder = Some(browse.with_state(action(strings.get("snippetsMoveTo"), "reorder")));
    }
    if ids.len() > end {
        section.more = Some(
            action(strings.get("snippetsLoadMore"), "browse")
                .arg("scope", browse.scope.id())
                .arg("query", browse.query.clone())
                .arg("page", (page + 1).to_string()),
        );
    }
    let preferences = ViewSection::new(
        None,
        vec![
            ViewItem::Preference {
                id: PREF_RECORD_PASTED.to_owned(),
            },
            ViewItem::Preference {
                id: PREF_REQUIRE_AUTH.to_owned(),
            },
        ],
    );
    ModuleView {
        sections: vec![section, preferences],
    }
}

/// What the snippet editor shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EditorValues {
    pub(crate) title: String,
    pub(crate) abbreviation: String,
    pub(crate) collection: Option<String>,
    pub(crate) body: String,
    pub(crate) template: bool,
    pub(crate) pinned: bool,
    pub(crate) concealed: bool,
}

impl EditorValues {
    fn values(&self) -> Map<String, Value> {
        let mut values = Map::new();
        values.insert("title".into(), self.title.clone().into());
        values.insert("abbreviation".into(), self.abbreviation.clone().into());
        values.insert(
            "collection".into(),
            json!([self
                .collection
                .as_deref()
                .map_or_else(|| UNFILED.to_owned(), collection_option)]),
        );
        values.insert("body".into(), self.body.clone().into());
        values.insert("template".into(), self.template.into());
        values.insert("pinned".into(), self.pinned.into());
        values.insert("concealed".into(), self.concealed.into());
        values
    }
}

pub(crate) struct Editor<'a> {
    pub(crate) id: Option<&'a str>,
    pub(crate) base: Option<&'a str>,
    pub(crate) values: &'a EditorValues,
    pub(crate) errors: BTreeMap<String, String>,
    pub(crate) notices: Vec<(String, NoticeTone)>,
    pub(crate) deleted: bool,
}

pub(crate) fn editor(
    strings: &Strings,
    collections: &[CollectionView],
    editor: Editor<'_>,
) -> ModuleView {
    let mut body = text_field(
        "body",
        strings.get("snippetsBodyLabel"),
        true,
        MAX_BODY_BYTES as u32,
    );
    if let ModuleFieldValue::Text {
        multiline,
        template,
        ..
    } = &mut body.value
    {
        *multiline = true;
        *template = true;
    }
    let fields = vec![
        text_field("title", strings.get("snippetsTitleLabel"), false, 1024),
        text_field(
            "abbreviation",
            strings.get("snippetsAbbreviationLabel"),
            false,
            256,
        ),
        body,
        choice_field(
            "collection",
            strings.get("snippetsCollectionLabel"),
            collection_choices(strings, collections, None),
        ),
        bool_field("template", strings.get("snippetsTemplateSwitch"), false),
        bool_field("pinned", strings.get("snippetsPinnedSwitch"), false),
        bool_field("concealed", strings.get("snippetsConcealedSwitch"), false),
    ];
    let id = editor.id.unwrap_or_default();
    let mut values = editor.values.values();
    if let Some(Value::Array(chosen)) = values.get("collection") {
        let known = chosen.iter().all(|choice| {
            choice.as_str().is_some_and(|choice| {
                collection_choices(strings, collections, None)
                    .iter()
                    .any(|(option, _)| option == choice)
            })
        });
        if !known {
            values.insert("collection".into(), json!([UNFILED]));
        }
    }
    let mut items: Vec<ViewItem> = editor
        .notices
        .into_iter()
        .map(|(text, tone)| ViewItem::Notice {
            text: short(&text, 1024),
            tone,
        })
        .collect();
    items.push(ViewItem::Form(ViewForm {
        fields,
        values,
        errors: editor.errors,
        submit: action(strings.get("commonSave"), "save")
            .arg("id", id)
            .arg("base", editor.base.unwrap_or_default()),
        preview: Some(action(strings.get("snippetsPreviewTab"), "preview").arg("id", id)),
    }));
    if !id.is_empty() {
        let actions = if editor.deleted {
            vec![action(strings.get("snippetsRestore"), "restore").arg("ids", json!([id]))]
        } else {
            vec![action(strings.get("commonDelete"), "delete").arg("ids", json!([id]))]
        };
        items.push(ViewItem::Toolbar { actions });
    }
    let title = if id.is_empty() {
        strings.get("snippetsNewSnippet")
    } else {
        strings.get("snippetsEditSnippet")
    };
    ModuleView {
        sections: vec![ViewSection::new(Some(title.to_owned()), items)],
    }
}

/// Asks whether templates inserting the old abbreviation follow the rename.
pub(crate) fn references_prompt(
    strings: &Strings,
    id: &str,
    base: &str,
    values: &Map<String, Value>,
    from: &str,
    to: &str,
) -> ModuleView {
    let save = |title: &str, references: &str| {
        action(title, "save")
            .arg("id", id)
            .arg("base", base)
            .arg("references", references)
            .arg("values", Value::Object(values.clone()))
    };
    ModuleView {
        sections: vec![ViewSection::new(
            Some(strings.get("snippetsUpdateReferencesTitle").to_owned()),
            vec![
                ViewItem::Notice {
                    text: short(
                        &strings.fill(
                            "snippetsUpdateReferencesMessage",
                            &[("from", from), ("to", to)],
                        ),
                        1024,
                    ),
                    tone: NoticeTone::Info,
                },
                ViewItem::Toolbar {
                    actions: vec![
                        save(strings.get("snippetsUpdateReferences"), "update"),
                        save(strings.get("snippetsKeepReferences"), "keep"),
                    ],
                },
            ],
        )],
    }
}

/// A choice id for a template field's option.
fn option_id(at: usize) -> String {
    format!("o{at}")
}

/// A form id for a template field.
pub(crate) fn field_id(at: usize) -> String {
    format!("f{at}")
}

/// The template's fill-in form, in Quick Paste.
pub(crate) fn fill(
    strings: &Strings,
    id: &str,
    title: &str,
    fields: &[TemplateField],
    preview: bool,
) -> ModuleView {
    let mut values = Map::new();
    let form_fields = fields
        .iter()
        .enumerate()
        .map(|(at, template)| {
            let form_id = field_id(at);
            let label = if template.label.trim().is_empty() {
                &template.name
            } else {
                &template.label
            };
            match template.kind {
                FieldKind::Choice if !template.options.is_empty() => {
                    let options: Vec<(String, String)> = template
                        .options
                        .iter()
                        .take(128)
                        .enumerate()
                        .map(|(index, option)| {
                            let title = if option.trim().is_empty() {
                                "—"
                            } else {
                                option
                            };
                            (option_id(index), title.to_owned())
                        })
                        .collect();
                    if let Some(at) = template
                        .default
                        .as_ref()
                        .and_then(|default| template.options.iter().position(|o| o == default))
                    {
                        values.insert(form_id.clone(), json!([option_id(at)]));
                    }
                    let mut field = choice_field(&form_id, label, options);
                    if let ModuleFieldValue::Choices { required, .. } = &mut field.value {
                        *required = template.required;
                    }
                    field
                }
                kind => {
                    if let Some(default) = &template.default {
                        values.insert(form_id.clone(), default.clone().into());
                    }
                    let mut field = text_field(&form_id, label, template.required, 64 * 1024);
                    if let ModuleFieldValue::Text { multiline, .. } = &mut field.value {
                        *multiline = kind == FieldKind::Multiline;
                    }
                    field
                }
            }
        })
        .collect();
    let form = ViewForm {
        fields: form_fields,
        values,
        errors: BTreeMap::new(),
        submit: action(strings.get("snippetsInsert"), "fill").arg("id", id),
        preview: preview
            .then(|| action(strings.get("snippetsPreviewTab"), "fill-preview").arg("id", id)),
    };
    ModuleView {
        sections: vec![ViewSection::new(
            Some(short(untitled(strings, title), 160)),
            vec![ViewItem::Form(form)],
        )],
    }
}

/// Template values from a submitted fill-in form, by field name.
pub(crate) fn fill_values(
    fields: &[TemplateField],
    submitted: &Map<String, Value>,
) -> BTreeMap<String, String> {
    fields
        .iter()
        .enumerate()
        .filter_map(|(at, template)| {
            let value = submitted.get(&field_id(at))?;
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Array(chosen) => {
                    let choice = chosen.first()?.as_str()?;
                    let index: usize = choice.strip_prefix('o')?.parse().ok()?;
                    template.options.get(index)?.clone()
                }
                _ => return None,
            };
            Some((template.name.clone(), text))
        })
        .collect()
}

pub(crate) fn move_form(
    strings: &Strings,
    collections: &[CollectionView],
    ids: &[String],
) -> ModuleView {
    form_view(
        strings.get("snippetsMoveTo"),
        ViewForm {
            fields: vec![choice_field(
                "collection",
                strings.get("snippetsCollectionLabel"),
                collection_choices(strings, collections, None),
            )],
            values: Map::from_iter([("collection".to_owned(), json!([UNFILED]))]),
            errors: BTreeMap::new(),
            submit: action(strings.get("snippetsMoveTo"), "move").arg("ids", json!(ids)),
            preview: None,
        },
        Vec::new(),
    )
}

fn form_view(title: &str, form: ViewForm, extra: Vec<ViewItem>) -> ModuleView {
    let mut items = vec![ViewItem::Form(form)];
    items.extend(extra);
    ModuleView {
        sections: vec![ViewSection::new(Some(short(title, 160)), items)],
    }
}

pub(crate) fn collection_editor(
    strings: &Strings,
    id: Option<&str>,
    name: &str,
    accent: &str,
    errors: BTreeMap<String, String>,
) -> ModuleView {
    let accent = if ACCENTS.contains(&accent) {
        accent
    } else {
        "neutral"
    };
    let form = ViewForm {
        fields: vec![
            text_field(
                "name",
                strings.get("snippetsCollectionName"),
                true,
                (MAX_COLLECTION_NAME_CHARS * 4) as u32,
            ),
            choice_field(
                "accent",
                strings.get("snippetsCollectionAccentLabel"),
                ACCENTS
                    .iter()
                    .map(|accent| {
                        (
                            (*accent).to_owned(),
                            strings.select("snippetsCollectionAccent", accent, &[]),
                        )
                    })
                    .collect(),
            ),
        ],
        values: Map::from_iter([
            ("name".to_owned(), Value::from(name)),
            ("accent".to_owned(), json!([accent])),
        ]),
        errors,
        submit: action(strings.get("commonSave"), "collection-save")
            .arg("id", id.unwrap_or_default()),
        preview: None,
    };
    let extra = id.map_or_else(Vec::new, |id| {
        vec![ViewItem::Toolbar {
            actions: vec![
                action(
                    strings.get("snippetsMergeCollection"),
                    "collection-merge-ask",
                )
                .arg("id", id),
                action(
                    strings.get("snippetsDeleteCollection"),
                    "collection-delete-ask",
                )
                .arg("id", id),
            ],
        }]
    });
    let title = if id.is_some() {
        strings.get("snippetsEditCollection")
    } else {
        strings.get("snippetsNewCollection")
    };
    form_view(title, form, extra)
}

pub(crate) fn collection_delete(strings: &Strings, id: &str, name: &str) -> ModuleView {
    ModuleView {
        sections: vec![ViewSection::new(
            Some(short(
                &strings.fill("snippetsDeleteCollectionTitle", &[("name", name)]),
                160,
            )),
            vec![ViewItem::Toolbar {
                actions: vec![
                    action(
                        strings.get("snippetsDeleteCollectionMove"),
                        "collection-delete",
                    )
                    .arg("id", id)
                    .arg("with_snippets", false),
                    action(
                        strings.get("snippetsDeleteCollectionWithSnippets"),
                        "collection-delete",
                    )
                    .arg("id", id)
                    .arg("with_snippets", true)
                    .destructive(),
                ],
            }],
        )],
    }
}

pub(crate) fn collection_merge(
    strings: &Strings,
    collections: &[CollectionView],
    id: &str,
    name: &str,
) -> ModuleView {
    let title = short(
        &strings.fill("snippetsMergeCollectionTitle", &[("name", name)]),
        160,
    );
    let choices = collection_choices(strings, collections, Some(id));
    if choices.is_empty() {
        return ModuleView {
            sections: vec![ViewSection::new(
                Some(title),
                vec![ViewItem::Notice {
                    text: strings.get("snippetsScopeEmpty").to_owned(),
                    tone: NoticeTone::Info,
                }],
            )],
        };
    }
    form_view(
        &title,
        ViewForm {
            fields: vec![choice_field(
                "into",
                strings.get("snippetsCollectionLabel"),
                choices,
            )],
            values: Map::new(),
            errors: BTreeMap::new(),
            submit: action(strings.get("snippetsMerge"), "collection-merge").arg("id", id),
            preview: None,
        },
        Vec::new(),
    )
}

pub(crate) fn import_form(
    strings: &Strings,
    values: Map<String, Value>,
    errors: BTreeMap<String, String>,
) -> ModuleView {
    let mut values = values;
    values.remove("passphrase");
    values.entry("convert").or_insert(Value::Bool(true));
    form_view(
        strings.get("snippetsImportTitle"),
        ViewForm {
            fields: vec![
                field(
                    "file",
                    strings.get("snippetsImportFileType"),
                    ModuleFieldValue::File {
                        accepted_extensions: vec!["copypaste-snippets".into(), "json".into(), "csv".into()],
                        required: true,
                        max_bytes: crate::bundle::MAX_IMPORT_BYTES,
                    },
                ),
                secret_field("passphrase", strings.get("snippetsImportPassphraseHint")),
                bool_field("convert", strings.get("snippetsImportConvert"), true),
            ],
            values,
            errors,
            submit: action(strings.get("snippetsImportPreview"), "import-preview"),
            preview: None,
        },
        Vec::new(),
    )
}

fn reason_text(strings: &Strings, reason: &str) -> String {
    let (code, detail) = reason.split_once(':').unwrap_or((reason, ""));
    match code {
        crate::import::REASON_ABBREVIATION_TAKEN => {
            strings.get("snippetsDuplicateAbbreviation").to_owned()
        }
        crate::import::REASON_INVALID_ABBREVIATION => {
            strings.get("snippetsAbbreviationWhitespace").to_owned()
        }
        crate::import::REASON_EMPTY_BODY => strings.get("snippetsFieldRequired").to_owned(),
        crate::import::REASON_TOO_LARGE => strings.get("snippetsSaveBlockTooLarge").to_owned(),
        crate::import::REASON_UNRECOGNIZED => {
            strings.select("snippetsLint", "unknownPlaceholder", &[("detail", detail)])
        }
        crate::import::REASON_LIBRARY_FULL => "The snippet library is full.".to_owned(),
        crate::import::REASON_INVALID_TITLE => "The title is too long or not valid.".to_owned(),
        _ => "The row is not valid.".to_owned(),
    }
}

pub(crate) const SAME_ID: [(&str, &str); 3] = [
    ("keep-both", "keepBoth"),
    ("replace", "replace"),
    ("skip", "skip"),
];
pub(crate) const ABBREVIATION: [(&str, &str); 3] = [
    ("import-without", "importWithout"),
    ("rename-with-suffix", "renameWithSuffix"),
    ("skip", "skip"),
];
pub(crate) const COLLECTIONS: [(&str, &str); 2] = [("merge", "merge"), ("create-new", "createNew")];

fn policy_field(
    strings: &Strings,
    id: &str,
    title: &str,
    key: &str,
    options: &[(&str, &str)],
) -> ModuleField {
    choice_field(
        id,
        title,
        options
            .iter()
            .map(|(id, option)| ((*id).to_owned(), strings.select(key, option, &[])))
            .collect(),
    )
}

pub(crate) fn import_preview(strings: &Strings, plan: &ImportPlan) -> ModuleView {
    let counts: ImportCounts = plan.counts();
    let mut items = vec![ViewItem::Notice {
        text: short(
            &strings.fill(
                "snippetsImportCounts",
                &[
                    ("added", &counts.new.to_string()),
                    ("identical", &counts.identical.to_string()),
                    ("updated", &counts.updated.to_string()),
                    ("conflicts", &counts.abbreviation_conflicts.to_string()),
                    ("invalid", &counts.invalid.to_string()),
                ],
            ),
            1024,
        ),
        tone: NoticeTone::Info,
    }];
    items.push(ViewItem::Form(ViewForm {
        fields: vec![
            policy_field(
                strings,
                "same_id",
                strings.get("snippetsImportSameIdPolicy"),
                "snippetsImportSameIdPolicyName",
                &SAME_ID,
            ),
            policy_field(
                strings,
                "abbreviation",
                strings.get("snippetsImportAbbreviationPolicy"),
                "snippetsImportAbbreviationPolicyName",
                &ABBREVIATION,
            ),
            policy_field(
                strings,
                "collections",
                strings.get("snippetsImportCollectionPolicy"),
                "snippetsImportCollectionPolicyName",
                &COLLECTIONS,
            ),
        ],
        values: Map::from_iter([
            ("same_id".to_owned(), json!(["keep-both"])),
            ("abbreviation".to_owned(), json!(["import-without"])),
            ("collections".to_owned(), json!(["merge"])),
        ]),
        errors: BTreeMap::new(),
        submit: action(strings.get("snippetsImport"), "import-commit").arg("token", plan.token()),
        preview: None,
    }));
    let mut sections = vec![ViewSection::new(
        Some(strings.get("snippetsImportTitle").to_owned()),
        items,
    )];
    let rows: Vec<ViewItem> = plan
        .rows()
        .into_iter()
        .filter(|row| row.outcome == ImportRowOutcome::Invalid || row.reason.is_some())
        .take(MAX_LISTED_ROWS)
        .map(|row| {
            let reason = row
                .reason
                .as_deref()
                .map(|reason| reason_text(strings, reason));
            ViewItem::row(
                short(
                    &strings.fill(
                        "snippetsImportInvalidRow",
                        &[
                            ("row", &(row.index + 1).to_string()),
                            ("reason", reason.as_deref().unwrap_or_default()),
                        ],
                    ),
                    256,
                ),
                None,
                Vec::new(),
            )
        })
        .collect();
    if !rows.is_empty() {
        sections.push(ViewSection::new(
            Some(strings.get("snippetsImportInvalidRows").to_owned()),
            rows,
        ));
    }
    ModuleView { sections }
}

pub(crate) fn export_form(
    strings: &Strings,
    scope: &str,
    ids: &[String],
    values: Map<String, Value>,
    errors: BTreeMap<String, String>,
) -> ModuleView {
    let mut values = values;
    values.remove("passphrase");
    values.remove("passphrase_confirm");
    let described = if !ids.is_empty() {
        strings.select(
            "snippetsExportScope",
            "snippets",
            &[("count", &ids.len().to_string())],
        )
    } else if scope.starts_with("c:") {
        strings.select("snippetsExportScope", "collections", &[])
    } else {
        strings.select("snippetsExportScope", "all", &[])
    };
    form_view(
        strings.get("snippetsExportTitle"),
        ViewForm {
            fields: vec![
                secret_field(
                    "passphrase",
                    &strings.fill("snippetsPassphraseHint", &[("min", "8")]),
                ),
                secret_field(
                    "passphrase_confirm",
                    strings.get("snippetsPassphraseConfirm"),
                ),
                bool_field(
                    "include_concealed",
                    strings.get("snippetsExportConcealedConfirm"),
                    false,
                ),
            ],
            values,
            errors,
            submit: action(strings.get("snippetsExport"), "export-run")
                .arg("scope", scope)
                .arg("ids", json!(ids)),
            preview: None,
        },
        vec![
            ViewItem::Notice {
                text: short(&described, 1024),
                tone: NoticeTone::Info,
            },
            ViewItem::Notice {
                text: strings.get("snippetsPassphraseNoRecovery").to_owned(),
                tone: NoticeTone::Warning,
            },
        ],
    )
}
