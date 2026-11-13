import 'dart:convert';

import 'package:characters/characters.dart';

/// Limits shared with the runtime (snippets spec 3.9).
abstract final class SnippetLimits {
  static const maxTitle = 200;
  static const maxAbbreviationGraphemes = 32;
  static const maxBodyBytes = 64 * 1024;
  static const maxCollectionName = 64;
  static const maxCollections = 200;
  static const maxFields = 10;
  static const pageSize = 50;
  static const pinnedRows = 20;

  /// A library this large shows a warning; the runtime refuses at 20,000.
  static const softLimit = 10000;
  static const minPassphrase = 8;

  /// Clips above this many UTF-8 bytes open read-only.
  static const maxEditableClipBytes = 256 * 1024;
}

enum SnippetScopeKind { all, unfiled, collection, pinned, recentlyDeleted }

/// Which snippets a list shows; [collectionId] is set for a collection.
class SnippetListScope {
  const SnippetListScope(this.kind, {this.collectionId})
    : assert((kind == SnippetScopeKind.collection) == (collectionId != null));

  const SnippetListScope.collection(String id)
    : this(SnippetScopeKind.collection, collectionId: id);

  static const all = SnippetListScope(SnippetScopeKind.all);
  static const unfiled = SnippetListScope(SnippetScopeKind.unfiled);
  static const pinned = SnippetListScope(SnippetScopeKind.pinned);
  static const recentlyDeleted = SnippetListScope(
    SnippetScopeKind.recentlyDeleted,
  );

  final SnippetScopeKind kind;
  final String? collectionId;

  @override
  bool operator ==(Object other) =>
      other is SnippetListScope &&
      other.kind == kind &&
      other.collectionId == collectionId;

  @override
  int get hashCode => Object.hash(kind, collectionId);
}

/// A snippet as listed. [preview] is empty for a concealed snippet.
class SnippetSummary {
  const SnippetSummary({
    required this.id,
    this.collectionId,
    this.title = '',
    this.abbreviation,
    this.template = false,
    this.pinned = false,
    this.concealed = false,
    this.preview = '',
    required this.createdAt,
    this.deletedAt,
    this.duplicateAbbreviation = false,
    this.conflictCopy = false,
    this.lastUsedAt,
  });

  final String id;
  final String? collectionId;
  final String title;
  final String? abbreviation;
  final bool template;
  final bool pinned;
  final bool concealed;
  final String preview;
  final DateTime createdAt;
  final DateTime? deletedAt;
  final bool duplicateAbbreviation;
  final bool conflictCopy;
  final DateTime? lastUsedAt;

  bool get deleted => deletedAt != null;
  bool get needsAttention => duplicateAbbreviation || conflictCopy;

  /// The title, else the first line of the preview (display only).
  String? get displayTitle {
    if (title.trim().isNotEmpty) return title.trim();
    final line = preview.trimLeft().split('\n').first.trim();
    return line.isEmpty ? null : line;
  }
}

class SnippetPage {
  const SnippetPage({
    required this.snippets,
    this.nextCursor,
    required this.total,
  });

  final List<SnippetSummary> snippets;
  final String? nextCursor;
  final int total;
}

enum TemplateFieldKind { text, multiline, choice }

class TemplateField {
  const TemplateField({
    required this.name,
    required this.label,
    this.kind = TemplateFieldKind.text,
    this.defaultValue,
    this.options = const [],
    this.required = false,
  });

  final String name;
  final String label;
  final TemplateFieldKind kind;
  final String? defaultValue;
  final List<String> options;
  final bool required;

  /// A choice without a valid default starts on its first option.
  String get initialValue => switch (kind) {
    TemplateFieldKind.choice =>
      options.contains(defaultValue)
          ? defaultValue!
          : (options.firstOrNull ?? ''),
    _ => defaultValue ?? '',
  };
}

enum TemplateLintSeverity { error, warning, hint }

/// [start] and [end] are UTF-8 byte offsets into the body.
class TemplateLint {
  const TemplateLint({
    required this.severity,
    required this.code,
    this.start = 0,
    this.end = 0,
    this.detail,
  });

  final TemplateLintSeverity severity;
  final String code;
  final int start;
  final int end;
  final String? detail;

  /// The UTF-16 offset of [start] in [body], for placing the caret.
  int textOffsetIn(String body) => utf16Offset(body, start);
}

/// The UTF-16 offset of the UTF-8 byte offset [byteOffset] in [text].
int utf16Offset(String text, int byteOffset) {
  var bytes = 0;
  var index = 0;
  for (final rune in text.runes) {
    if (bytes >= byteOffset) break;
    bytes += utf8.encode(String.fromCharCode(rune)).length;
    index += rune > 0xFFFF ? 2 : 1;
  }
  return index;
}

class SnippetDetail {
  const SnippetDetail({
    required this.summary,
    required this.bodyText,
    this.bodyRich,
    this.revision = '',
    this.fields = const [],
    this.lints = const [],
  });

  final SnippetSummary summary;
  final String bodyText;
  final String? bodyRich;
  final String revision;
  final List<TemplateField> fields;
  final List<TemplateLint> lints;

  String get id => summary.id;
}

class SnippetDraft {
  const SnippetDraft({
    this.id,
    this.collectionId,
    this.title = '',
    this.abbreviation,
    required this.bodyText,
    this.bodyRich,
    this.template = false,
    this.pinned = false,
    this.concealed = false,
    this.baseRevision,
  });

  final String? id;
  final String? collectionId;
  final String title;
  final String? abbreviation;
  final String bodyText;
  final String? bodyRich;
  final bool template;
  final bool pinned;
  final bool concealed;
  final String? baseRevision;
}

sealed class SnippetSaveOutcome {
  const SnippetSaveOutcome();
}

class SnippetSaved extends SnippetSaveOutcome {
  const SnippetSaved(this.detail);

  final SnippetDetail detail;
}

/// The normalized abbreviation belongs to another live snippet.
class SnippetAbbreviationTaken extends SnippetSaveOutcome {
  const SnippetAbbreviationTaken({
    required this.snippetId,
    required this.title,
  });

  final String snippetId;
  final String title;
}

/// Icons a collection may carry, stored by name.
enum SnippetCollectionIcon {
  folder,
  briefcase,
  house,
  code,
  mail,
  messageSquare,
  star,
  bookmark,
  user,
  heart,
}

/// Accents from the shared theme's status palette, stored by name.
enum SnippetCollectionAccent { neutral, blue, green, amber, red }

class SnippetCollection {
  const SnippetCollection({
    required this.id,
    required this.name,
    String? displayName,
    this.icon = SnippetCollectionIcon.folder,
    this.accent = SnippetCollectionAccent.neutral,
    this.snippetCount = 0,
  }) : displayName = displayName ?? name;

  final String id;
  final String name;

  /// [name] with a suffix when two devices created the same name.
  final String displayName;
  final SnippetCollectionIcon icon;
  final SnippetCollectionAccent accent;
  final int snippetCount;
}

class SnippetCollectionDraft {
  const SnippetCollectionDraft({
    this.id,
    required this.name,
    this.icon = SnippetCollectionIcon.folder,
    this.accent = SnippetCollectionAccent.neutral,
  });

  final String? id;
  final String name;
  final SnippetCollectionIcon icon;
  final SnippetCollectionAccent accent;
}

enum SnippetMatchTier { exactAbbreviation, abbreviationPrefix, title, fullText }

class SnippetHit {
  const SnippetHit({
    required this.snippet,
    required this.tier,
    this.duplicate = false,
  });

  final SnippetSummary snippet;
  final SnippetMatchTier tier;
  final bool duplicate;
}

/// Where the `clipboard` placeholder reads from.
enum TemplateClipboardSource {
  /// The current clipboard, read once when rendering starts.
  host,

  /// Nothing, for previews that must not read the clipboard.
  none,
}

class RenderedTemplate {
  const RenderedTemplate({
    required this.text,
    this.cursorFromEnd,
    this.warnings = const [],
    this.missingRequired = const [],
    this.unfilled = 0,
  });

  final String text;
  final int? cursorFromEnd;
  final List<TemplateLint> warnings;
  final List<String> missingRequired;

  /// Placeholders left verbatim.
  final int unfilled;
}

enum SnippetDelivery { paste, copy, returnText }

class SnippetDelivered {
  const SnippetDelivered({this.token, this.text, this.cursorFromEnd});

  final String? token;
  final String? text;
  final int? cursorFromEnd;
}

enum SnippetImportFormat { bundle, json, csv }

enum SnippetImportRowOutcome {
  added,
  identical,
  updated,
  abbreviationConflict,
  invalid,
}

class SnippetImportRow {
  const SnippetImportRow({
    required this.index,
    required this.outcome,
    this.reason,
  });

  final int index;
  final SnippetImportRowOutcome outcome;
  final String? reason;
}

class SnippetImportCounts {
  const SnippetImportCounts({
    this.added = 0,
    this.identical = 0,
    this.updated = 0,
    this.abbreviationConflicts = 0,
    this.invalid = 0,
    this.collections = 0,
  });

  final int added;
  final int identical;
  final int updated;
  final int abbreviationConflicts;
  final int invalid;
  final int collections;

  int get importable => added + updated + abbreviationConflicts;
}

class SnippetImportPlan {
  const SnippetImportPlan({
    required this.token,
    required this.counts,
    this.rows = const [],
  });

  final String token;
  final SnippetImportCounts counts;
  final List<SnippetImportRow> rows;
}

enum SnippetSameIdPolicy { keepBoth, replace, skip }

enum SnippetAbbreviationPolicy { importWithout, renameWithSuffix, skip }

enum SnippetCollectionNamePolicy { merge, createNew }

class SnippetImportPolicies {
  const SnippetImportPolicies({
    this.sameIdDifferent = SnippetSameIdPolicy.keepBoth,
    this.abbreviationConflict = SnippetAbbreviationPolicy.importWithout,
    this.collectionName = SnippetCollectionNamePolicy.merge,
  });

  final SnippetSameIdPolicy sameIdDifferent;
  final SnippetAbbreviationPolicy abbreviationConflict;
  final SnippetCollectionNamePolicy collectionName;

  SnippetImportPolicies copyWith({
    SnippetSameIdPolicy? sameIdDifferent,
    SnippetAbbreviationPolicy? abbreviationConflict,
    SnippetCollectionNamePolicy? collectionName,
  }) => SnippetImportPolicies(
    sameIdDifferent: sameIdDifferent ?? this.sameIdDifferent,
    abbreviationConflict: abbreviationConflict ?? this.abbreviationConflict,
    collectionName: collectionName ?? this.collectionName,
  );
}

class SnippetImportReport {
  const SnippetImportReport({
    this.inserted = 0,
    this.updated = 0,
    this.skipped = 0,
    this.renamedAbbreviations = 0,
    this.withoutAbbreviation = 0,
    this.collectionsCreated = 0,
    this.unrecognizedPlaceholders = 0,
  });

  final int inserted;
  final int updated;
  final int skipped;
  final int renamedAbbreviations;
  final int withoutAbbreviation;
  final int collectionsCreated;
  final int unrecognizedPlaceholders;
}

enum SnippetExportScopeKind { all, collections, snippets }

class SnippetExportRequest {
  const SnippetExportRequest({
    required this.path,
    this.scope = SnippetExportScopeKind.all,
    this.ids = const [],
    this.passphrase,
    this.includeConcealed = false,
  });

  final String path;
  final SnippetExportScopeKind scope;
  final List<String> ids;

  /// Null writes a plain bundle anyone can read.
  final String? passphrase;
  final bool includeConcealed;
}

/// Synced snippet preferences; authentication itself runs per device.
class SnippetPreferences {
  const SnippetPreferences({
    this.recordPasted = false,
    this.requireAuth = false,
  });

  final bool recordPasted;
  final bool requireAuth;
}

enum SnippetPreference { recordPasted, requireAuth }

/// A runtime refusal; [message] is safe to show.
class SnippetException implements Exception {
  const SnippetException(this.code, this.message);

  static const notFound = 'not_found';

  final String code;
  final String message;

  @override
  String toString() => 'SnippetException($code)';
}

enum AbbreviationProblem { tooLong, whitespace }

/// Local syntax of an abbreviation (spec A-1); uniqueness is the runtime's.
AbbreviationProblem? abbreviationProblem(String abbreviation) {
  if (abbreviation.characters.length > SnippetLimits.maxAbbreviationGraphemes) {
    return AbbreviationProblem.tooLong;
  }
  final invalid = RegExp(r'[\s\x00-\x1F\x7F-\x9F]');
  if (invalid.hasMatch(abbreviation)) return AbbreviationProblem.whitespace;
  return null;
}

/// Bodies are stored with LF line endings.
String normalizeLineEndings(String text) =>
    text.replaceAll('\r\n', '\n').replaceAll('\r', '\n');

int utf8Length(String text) => utf8.encode(text).length;
