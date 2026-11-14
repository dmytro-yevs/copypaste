import 'dart:convert';

abstract final class TransformLimits {
  /// Longest chain a palette, pipeline or run accepts.
  static const maxSteps = 16;

  /// The shown prefix of each preview pane.
  static const previewBytes = 64 * 1024;
}

/// Catalogue category keys, in palette order.
enum TransformCategory {
  structure('structure'),
  encode('encode'),
  caseChange('case'),
  hash('hash'),
  lines('lines'),
  clean('clean'),
  url('url'),
  markup('markup'),
  dev('dev'),
  other('');

  const TransformCategory(this.key);

  final String key;

  static TransformCategory fromKey(String key) =>
      values.firstWhere((category) => category.key == key, orElse: () => other);

  /// Categories whose text reads best in a monospace preview.
  bool get monospace =>
      this == structure || this == encode || this == hash || this == dev;
}

enum TransformParamKind { choice, boolean, integer, text }

class TransformParamOption {
  const TransformParamOption({required this.value, required this.label});

  final String value;
  final String label;
}

class TransformParam {
  const TransformParam({
    required this.key,
    required this.label,
    required this.kind,
    required this.defaultValue,
    this.options = const [],
    this.min,
    this.max,
    this.maxLength,
  });

  final String key;
  final String label;
  final TransformParamKind kind;
  final Object? defaultValue;
  final List<TransformParamOption> options;
  final int? min;
  final int? max;

  /// Longest text value in UTF-8 bytes.
  final int? maxLength;

  /// Whether [value] fits this parameter, as the runtime checks it.
  bool accepts(Object? value) => switch (kind) {
    TransformParamKind.boolean => value is bool,
    TransformParamKind.choice =>
      value is String && options.any((option) => option.value == value),
    TransformParamKind.integer =>
      value is int &&
          (min == null || value >= min!) &&
          (max == null || value <= max!),
    TransformParamKind.text =>
      value is String &&
          (maxLength == null || utf8.encode(value).length <= maxLength!),
  };
}

class TransformDefinition {
  const TransformDefinition({
    required this.id,
    required this.version,
    required this.category,
    required this.name,
    required this.description,
    this.exampleIn = '',
    this.exampleOut = '',
    this.keywords = const [],
    this.params = const [],
    this.canBeDefault = true,
    this.needsFreeText = false,
    this.sensitive = false,
    this.richInput = false,
    this.deterministic = true,
  });

  final String id;
  final int version;
  final TransformCategory category;
  final String name;
  final String description;
  final String exampleIn;
  final String exampleOut;
  final List<String> keywords;
  final List<TransformParam> params;
  final bool canBeDefault;
  final bool needsFreeText;

  /// Output is always sensitive, such as a decoded token.
  final bool sensitive;
  final bool richInput;
  final bool deterministic;

  TransformStep step([Map<String, Object?> params = const {}]) =>
      TransformStep(transformId: id, version: version, params: params);
}

/// One step of a chain; [params] holds only values that differ from the
/// defaults the user expects to apply.
class TransformStep {
  const TransformStep({
    required this.transformId,
    required this.version,
    this.params = const {},
    this.enabled = true,
  });

  factory TransformStep.fromJson(Map<String, Object?> json) => TransformStep(
    transformId: json['transform_id'] as String,
    version: json['version'] as int,
    params: Map<String, Object?>.from(json['params'] as Map? ?? const {}),
    enabled: json['enabled'] as bool? ?? true,
  );

  final String transformId;
  final int version;
  final Map<String, Object?> params;
  final bool enabled;

  String get paramsJson => jsonEncode(params);

  TransformStep copyWith({Map<String, Object?>? params, bool? enabled}) =>
      TransformStep(
        transformId: transformId,
        version: version,
        params: params ?? this.params,
        enabled: enabled ?? this.enabled,
      );

  Map<String, Object?> toJson() => {
    'transform_id': transformId,
    'version': version,
    'params': params,
    'enabled': enabled,
  };
}

/// What a palette transforms. Opening the palette never reads the system
/// clipboard unless the source is [TransformSource.clipboard].
sealed class TransformSource {
  const TransformSource();

  const factory TransformSource.clip(String id, {bool text}) =
      TransformClipSource;
  const factory TransformSource.text(String text) = TransformTextSource;
  const factory TransformSource.clipboard() = TransformClipboardSource;
}

class TransformClipSource extends TransformSource {
  const TransformClipSource(this.id, {this.text = true});

  final String id;

  /// False for an image, file or color clip, which transforms cannot read.
  final bool text;
}

class TransformTextSource extends TransformSource {
  const TransformTextSource(this.text);

  final String text;
}

class TransformClipboardSource extends TransformSource {
  const TransformClipboardSource();
}

class TextCounts {
  const TextCounts({
    this.chars = 0,
    this.words = 0,
    this.lines = 0,
    this.bytes = 0,
  });

  final int chars;
  final int words;
  final int lines;

  /// Size of the whole text, even when only a prefix is shown.
  final int bytes;
}

/// Why a step failed: a stable code and, for input errors, where.
class TransformIssue {
  const TransformIssue({
    required this.code,
    this.offset,
    this.line,
    this.column,
  });

  final String code;
  final int? offset;
  final int? line;
  final int? column;
}

class TransformWarning {
  const TransformWarning({required this.code, this.detail});

  final String code;

  /// `line:col` when the warning points into the input.
  final String? detail;
}

class TransformStepPreview {
  const TransformStepPreview({
    required this.output,
    this.counts = const TextCounts(),
    this.warnings = const [],
    this.failure,
    this.notRun = false,
  });

  /// At most the first 64 KiB of the step's output.
  final String output;
  final TextCounts counts;
  final List<TransformWarning> warnings;
  final TransformIssue? failure;

  /// Disabled, or after a failed step.
  final bool notRun;
}

class TransformPreview {
  const TransformPreview({
    required this.inputCounts,
    required this.steps,
    this.input,
    this.truncated = false,
    this.noChanges = false,
    this.sensitive = false,
  });

  final TextCounts inputCounts;

  /// The shown prefix of the input, when the preview asked for it.
  final String? input;
  final List<TransformStepPreview> steps;

  /// The input or an output is longer than the shown prefix.
  final bool truncated;
  final bool noChanges;
  final bool sensitive;

  int? get failedStep {
    final index = steps.indexWhere((step) => step.failure != null);
    return index < 0 ? null : index;
  }

  TransformIssue? get failure =>
      failedStep == null ? null : steps[failedStep!].failure;
}

enum TransformRunAction { paste, copy, saveNewClip }

class TransformRunResult {
  const TransformRunResult({this.restoreToken, this.clipId});

  /// Present after a transient paste while clipboard restore is on.
  final String? restoreToken;

  /// The clip a copy or save created.
  final String? clipId;
}

class TransformPipeline {
  const TransformPipeline({
    required this.id,
    required this.name,
    this.description,
    this.steps = const [],
  });

  final String id;
  final String name;
  final String? description;
  final List<TransformStep> steps;
}

class TransformPipelineDraft {
  const TransformPipelineDraft({
    this.id,
    required this.name,
    this.description,
    required this.steps,
  });

  final String? id;
  final String name;
  final String? description;
  final List<TransformStep> steps;
}

enum TransformFavoriteKind { transform, pipeline }

class TransformFavorite {
  const TransformFavorite(this.kind, this.id);

  final TransformFavoriteKind kind;
  final String id;

  @override
  bool operator ==(Object other) =>
      other is TransformFavorite && other.kind == kind && other.id == id;

  @override
  int get hashCode => Object.hash(kind, id);

  Map<String, Object?> toJson() => {'kind': kind.name, 'id': id};
}

/// A default paste pipeline for one application on one platform;
/// [pipelineId] null means "no default" for that application.
class TransformAppDefault {
  const TransformAppDefault({
    required this.platform,
    required this.identifier,
    this.pipelineId,
  });

  final String platform;
  final String identifier;
  final String? pipelineId;
}

/// Synced sets of transform preferences, by wire name.
enum TransformSet {
  hidden('hidden_transforms'),
  trackingRemove('tracking_remove'),
  trackingAllow('tracking_allow'),
  disabledDetectors('disabled_detectors');

  const TransformSet(this.wireName);

  final String wireName;
}

/// Content detectors that drive palette suggestions, by wire name.
enum TransformDetector {
  json('json'),
  jwt('jwt'),
  base64('base64'),
  trackingUrl('tracking_url'),
  timestamp('timestamp'),
  html('html'),
  markdown('markdown');

  const TransformDetector(this.wireName);

  final String wireName;
}

class TransformPreferences {
  const TransformPreferences({
    this.favorites = const [],
    this.hidden = const {},
    this.paramDefaults = const {},
    this.defaultPastePipelineId,
    this.appDefaults = const [],
    this.trackingRemove = const [],
    this.trackingAllow = const [],
    this.restoreClipboardAfterPaste = true,
    this.suggestionsEnabled = true,
    this.disabledDetectors = const {},
  });

  final List<TransformFavorite> favorites;
  final Set<String> hidden;
  final Map<String, Map<String, Object?>> paramDefaults;
  final String? defaultPastePipelineId;
  final List<TransformAppDefault> appDefaults;
  final List<String> trackingRemove;
  final List<String> trackingAllow;
  final bool restoreClipboardAfterPaste;
  final bool suggestionsEnabled;
  final Set<String> disabledDetectors;

  bool isFavorite(TransformFavorite favorite) => favorites.contains(favorite);
}

/// A runtime refusal with its stable error code.
class TransformException implements Exception {
  const TransformException(this.code, this.message);

  static const unsupportedContent = 'unsupported_content';
  static const contentTooLarge = 'content_too_large';
  static const notFound = 'not_found';
  static const unsupported = 'unsupported';

  final String code;
  final String message;

  @override
  String toString() => 'TransformException($code)';
}
