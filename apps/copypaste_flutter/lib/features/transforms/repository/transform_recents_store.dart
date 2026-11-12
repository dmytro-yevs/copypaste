import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

import '../models/transform_models.dart';

/// A chain the user ran: its steps, or a saved pipeline by id.
class TransformRecent {
  const TransformRecent({this.steps = const [], this.pipelineId});

  final List<TransformStep> steps;
  final String? pipelineId;

  static TransformRecent? fromJson(Object? json) {
    if (json is! Map) return null;
    final pipelineId = json['pipeline_id'];
    final steps = json['steps'];
    try {
      return TransformRecent(
        pipelineId: pipelineId is String ? pipelineId : null,
        steps: [
          if (steps is List)
            for (final step in steps)
              TransformStep.fromJson(Map<String, Object?>.from(step as Map)),
        ],
      );
    } on Object {
      return null;
    }
  }

  Map<String, Object?> toJson() => {
    'pipeline_id': ?pipelineId,
    'steps': [for (final step in steps) step.toJson()],
  };

  /// The key that makes two recents the same entry.
  String get identity => pipelineId ?? jsonEncode(toJson()['steps']);
}

/// Device-local recent chains, newest first. Recents and "repeat last" are
/// not synced, so using a transform never writes a library record.
abstract interface class TransformRecentsStore {
  Future<List<TransformRecent>> read();

  Future<void> write(List<TransformRecent> recents);
}

class SharedPreferencesTransformRecentsStore implements TransformRecentsStore {
  SharedPreferencesTransformRecentsStore({SharedPreferencesAsync? preferences})
    : _preferences = preferences ?? SharedPreferencesAsync();

  static const _key = 'transforms.recents.v1';

  final SharedPreferencesAsync _preferences;

  @override
  Future<List<TransformRecent>> read() async {
    try {
      final encoded = await _preferences.getString(_key);
      if (encoded == null) return const [];
      final decoded = jsonDecode(encoded);
      if (decoded is! List) return const [];
      return [for (final entry in decoded) ?TransformRecent.fromJson(entry)];
    } on Object {
      return const [];
    }
  }

  @override
  Future<void> write(List<TransformRecent> recents) async {
    try {
      await _preferences.setString(
        _key,
        jsonEncode([for (final recent in recents) recent.toJson()]),
      );
    } on Object {
      // Recents are a convenience; a failed write keeps them in memory only.
    }
  }
}

/// Recents for tests and engines without persistent preferences.
class MemoryTransformRecentsStore implements TransformRecentsStore {
  List<TransformRecent> _recents = const [];

  @override
  Future<List<TransformRecent>> read() async => _recents;

  @override
  Future<void> write(List<TransformRecent> recents) async =>
      _recents = List.unmodifiable(recents);
}
