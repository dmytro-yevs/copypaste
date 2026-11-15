import 'package:flutter/foundation.dart';

import '../models/transform_models.dart';

/// Runtime access for transform palettes and transform settings.
///
/// Failures are [TransformException]s carrying the runtime error code.
abstract interface class TransformRepository {
  /// Names and descriptions in [locale]; loaded per palette session.
  Future<List<TransformDefinition>> catalogue(String locale);

  /// A bounded preview; a newer preview supersedes an older one.
  /// [withInput] also returns the shown prefix of the input.
  Future<TransformPreview> preview(
    String requestId,
    TransformSource source,
    List<TransformStep> steps, {
    bool withInput = false,
  });

  Future<void> cancel(String requestId);

  /// Runs [steps], or the saved pipeline [pipelineId] when it is set.
  /// [bypassDefault] delivers the source as itself.
  Future<TransformRunResult> run(
    TransformSource source, {
    List<TransformStep> steps = const [],
    String? pipelineId,
    required TransformRunAction action,
    bool bypassDefault = false,
  });

  /// Up to three suggested transform ids for [source].
  Future<List<String>> suggest(TransformSource source);

  Future<List<TransformPipeline>> pipelines();

  Future<TransformPipeline> savePipeline(TransformPipelineDraft draft);

  Future<void> deletePipeline(String id);

  Future<TransformPreferences> preferences();

  /// The pipeline a paste into [identifier] on [platform] goes through: that
  /// application's own default, else the global one. A null [identifier]
  /// (target unknown) answers the global default.
  Future<String?> defaultPipelineFor(String platform, String? identifier);

  /// Sets one synced preference; `null` clears a keyed override.
  Future<TransformPreferences> setPreference(String key, Object? value);

  Future<TransformPreferences> setMember(
    TransformSet set,
    String member, {
    required bool present,
  });

  /// Puts the clipboard back after a transient paste, unless it changed.
  Future<void> restoreClipboard(String token, Duration after);

  /// Signals that pipelines or preferences changed, here or on a peer.
  Stream<void> changes();
}

/// Preference keys the runtime accepts.
abstract final class TransformPreferenceKey {
  static const favorites = 'favorites';
  static const defaultPastePipeline = 'default_paste_pipeline_id';
  static const restoreClipboard = 'restore_clipboard_after_paste';
  static const suggestions = 'suggestions_enabled';

  static String paramDefaults(String transformId) =>
      'param_defaults:$transformId';

  static String appDefault(String platform, String identifier) =>
      'app_default:$platform:$identifier';

  /// The platform name of [target] in per-app default keys.
  static String platformName(TargetPlatform target) => switch (target) {
    TargetPlatform.macOS => 'macos',
    TargetPlatform.windows => 'windows',
    TargetPlatform.android => 'android',
    TargetPlatform.iOS => 'ios',
    TargetPlatform.fuchsia => 'fuchsia',
    TargetPlatform.linux => 'linux',
  };
}
