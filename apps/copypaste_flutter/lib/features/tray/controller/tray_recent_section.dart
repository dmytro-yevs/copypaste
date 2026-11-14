import '../../../platform/security/device_authenticator.dart';
import '../models/tray_menu_model.dart';
import '../repository/tray_clip_source.dart';

/// Loads the menu's on-demand rows (Recent, the latest clip from another
/// device, recently skipped captures) only when they are about to be seen,
/// and drops the clip previews again when the menu closes.
class TrayRecentSection {
  TrayRecentSection({
    required TrayClipSource source,
    required DeviceAuthenticator authenticator,
  }) : _source = source,
       _authenticator = authenticator;

  final TrayClipSource _source;
  final DeviceAuthenticator _authenticator;
  int _generation = 0;

  /// Whether each clip the menu last offered was masked. Only ids are kept,
  /// so a choice made after the menu closed is still checked correctly.
  final Map<String, bool> _offered = {};

  /// Whether choosing [clipId] needs device authentication. A clip this
  /// section never offered is treated as masked.
  bool requiresUnlock(String clipId) => _offered[clipId] ?? true;

  TrayMenuState loading(TrayMenuState state) =>
      state.copyWith(recentStatus: TrayLoadStatus.loading);

  /// Reads the rows for [limit] clips. Resolves to null when a newer load or
  /// a release superseded this one.
  Future<TrayMenuState Function(TrayMenuState)?> load(int limit) async {
    final generation = ++_generation;
    final results = await Future.wait<Object?>([
      _source.recent(limit).then<Object?>((clips) => clips, onError: _none),
      _source.latestFromOtherDevice().then<Object?>(
        (clip) => clip,
        onError: _none,
      ),
      _source.recentlySkipped().then<Object?>(
        (entries) => entries,
        onError: _none,
      ),
      _authenticator.availability().then<Object?>(
        (availability) => availability,
        onError: _none,
      ),
    ]);
    if (generation != _generation) return null;
    final recent = results[0] as List<TrayClip>?;
    final latest = results[1] as TrayClip?;
    final skipped = results[2] as List<TraySkipEntry>? ?? const [];
    final copyable = results[3] == DeviceAuthAvailability.available;
    _offered
      ..clear()
      ..addEntries([
        for (final clip in recent ?? const <TrayClip>[])
          MapEntry(clip.id, clip.masked),
      ]);
    return (TrayMenuState state) => state.copyWith(
      recentStatus: recent == null
          ? TrayLoadStatus.failed
          : TrayLoadStatus.ready,
      recent: recent ?? const [],
      latestFromOtherDevice: latest == null
          ? null
          : TrayClip(
              id: latest.id,
              kind: latest.kind,
              masked: latest.masked,
              capturedAt: latest.capturedAt,
            ),
      skipped: skipped.take(trayMaxSkipEntries).toList(growable: false),
      secretsCopyable: copyable,
    );
  }

  /// Drops clip previews; any load still running is discarded.
  TrayMenuState release(TrayMenuState state) {
    _generation += 1;
    return state.copyWith(recentStatus: TrayLoadStatus.idle, recent: const []);
  }

  static Object? _none(Object _) => null;
}
