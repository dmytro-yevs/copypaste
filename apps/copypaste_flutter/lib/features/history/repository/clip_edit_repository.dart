import '../../../generated/api.dart' as runtime;
import '../../../generated/api/clips.dart' as clips;
import '../../../shared/error_text.dart';

/// Text clip creation and editing for History.
///
/// Failures are [ClipEditException]s carrying the runtime error code.
abstract interface class ClipEditRepository {
  /// A new history clip attributed to CopyPaste with a manual marker.
  Future<String> createManualClip(String text);

  /// Saves [text] as a new clip marked edited; [replaceOriginal] deletes the
  /// original after the new clip is saved. Returns the resulting clip id.
  Future<String> saveClipEdit(
    String id,
    String text, {
    required bool replaceOriginal,
  });
}

class ClipEditException implements RuntimeFailure {
  const ClipEditException(this.code, [this.message = '']);

  @override
  final String code;
  @override
  final String message;

  @override
  String toString() => 'ClipEditException($code)';
}

/// [ClipEditRepository] over the runtime bridge.
class RuntimeClipEditRepository implements ClipEditRepository {
  const RuntimeClipEditRepository();

  @override
  Future<String> createManualClip(String text) =>
      _guard(() async => (await clips.createManualClip(text: text)).id);

  @override
  Future<String> saveClipEdit(
    String id,
    String text, {
    required bool replaceOriginal,
  }) => _guard(
    () async => (await clips.saveClipEdit(
      id: id,
      text: text,
      replaceOriginal: replaceOriginal,
    )).id,
  );

  static Future<T> _guard<T>(Future<T> Function() request) async {
    try {
      return await request();
    } on runtime.RuntimeError catch (error) {
      throw ClipEditException(error.code, error.message);
    }
  }
}
