import '../models/transform_models.dart';
import '../repository/transform_recents_store.dart';

enum TransformEntryKind { transform, pipeline, recent }

/// One row of the transform palette: a transform, a saved pipeline, or a
/// recently run chain.
class TransformEntry {
  const TransformEntry.transform(TransformDefinition this.definition)
    : kind = TransformEntryKind.transform,
      pipeline = null,
      recent = null,
      unavailable = false;

  const TransformEntry.pipeline(
    TransformPipeline this.pipeline, {
    this.unavailable = false,
  }) : kind = TransformEntryKind.pipeline,
       definition = null,
       recent = null;

  const TransformEntry.recent(TransformRecent this.recent)
    : kind = TransformEntryKind.recent,
      definition = null,
      pipeline = null,
      unavailable = false;

  final TransformEntryKind kind;
  final TransformDefinition? definition;
  final TransformPipeline? pipeline;
  final TransformRecent? recent;

  /// A pipeline step needs a newer CopyPaste version, so it cannot run.
  final bool unavailable;

  String get id => switch (kind) {
    TransformEntryKind.transform => 'transform:${definition!.id}',
    TransformEntryKind.pipeline => 'pipeline:${pipeline!.id}',
    TransformEntryKind.recent => 'recent:${recent!.identity}',
  };

  TransformFavorite? get favorite => switch (kind) {
    TransformEntryKind.transform => TransformFavorite(
      TransformFavoriteKind.transform,
      definition!.id,
    ),
    TransformEntryKind.pipeline => TransformFavorite(
      TransformFavoriteKind.pipeline,
      pipeline!.id,
    ),
    TransformEntryKind.recent => null,
  };

  @override
  bool operator ==(Object other) => other is TransformEntry && other.id == id;

  @override
  int get hashCode => id.hashCode;
}

/// A titled group of palette rows.
class TransformEntryGroup {
  const TransformEntryGroup(this.kind, this.entries, {this.category});

  final TransformEntryGroupKind kind;
  final List<TransformEntry> entries;

  /// Set for [TransformEntryGroupKind.category].
  final TransformCategory? category;
}

enum TransformEntryGroupKind {
  results,
  suggested,
  recent,
  favorites,
  pipelines,
  category,
}

/// Ranks [entries] for [query]: a name prefix first, then a name or
/// keyword match, then a subsequence of the name. Every query word must
/// match somewhere in the name, keywords, category or description.
List<TransformEntry> rankTransformEntries(
  Iterable<TransformEntry> entries,
  String query,
  String Function(TransformEntry entry) name,
  String Function(TransformEntry entry) haystack,
) {
  final needle = query.trim().toLowerCase();
  if (needle.isEmpty) return entries.toList(growable: false);
  final words = needle.split(RegExp(r'\s+'));
  final scored = <(int, int, TransformEntry)>[];
  var order = 0;
  for (final entry in entries) {
    final title = name(entry).toLowerCase();
    final text = '$title ${haystack(entry).toLowerCase()}';
    final int score;
    if (title.startsWith(needle)) {
      score = 0;
    } else if (words.every(text.contains)) {
      score = title.contains(needle) ? 1 : 2;
    } else if (_subsequence(needle.replaceAll(' ', ''), title)) {
      score = 3;
    } else {
      order++;
      continue;
    }
    scored.add((score, order++, entry));
  }
  scored.sort((a, b) => a.$1 != b.$1 ? a.$1 - b.$1 : a.$2 - b.$2);
  return [for (final (_, _, entry) in scored) entry];
}

bool _subsequence(String needle, String text) {
  var index = 0;
  for (final unit in text.codeUnits) {
    if (index < needle.length && needle.codeUnitAt(index) == unit) index++;
  }
  return index == needle.length;
}
