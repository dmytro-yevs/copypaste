import 'package:flutter/foundation.dart';

import '../../history/models/history_models.dart';
import '../models/application_models.dart';
import '../repository/application_catalog.dart';

/// Application names and icons for one view: installed apps merged with apps
/// seen in History on this platform. Found names and icons are memoized until
/// the owning view drops this directory; misses are retried after
/// [missLifetime], and the History list is held only while a lookup needs it.
class ApplicationDirectory {
  ApplicationDirectory(
    this._catalog, {
    TargetPlatform? platform,
    DateTime Function() clock = DateTime.now,
  }) : _platform = platform ?? defaultTargetPlatform,
       _clock = clock;

  static const missLifetime = Duration(minutes: 1);

  final ApplicationCatalog _catalog;
  final TargetPlatform _platform;
  final DateTime Function() _clock;
  Future<List<ApplicationChoice>>? _seen;
  var _seenUsers = 0;
  final _described = <String, _Lookup<ApplicationChoice>>{};
  final _icons = <String, _Lookup<HistorySourceAppIcon>>{};

  /// The key every spelling of one application's identifier shares. Windows
  /// image names compare without case or `.exe`, as capture exclusions do.
  String key(String id) {
    if (_platform != TargetPlatform.windows) return id;
    final lower = id.trim().toLowerCase();
    return lower.endsWith('.exe')
        ? lower.substring(0, lower.length - '.exe'.length)
        : lower;
  }

  /// Whether [id] has the form capture attribution records on this platform:
  /// a Windows image name, a macOS bundle ID, an Android package name or a
  /// Linux desktop entry ID. History synced from other platforms holds the
  /// others, which can never match a capture here.
  bool attributable(String id) {
    final windowsImage = _windowsImage.hasMatch(id);
    return switch (_platform) {
      TargetPlatform.windows => windowsImage,
      TargetPlatform.macOS ||
      TargetPlatform.iOS => !windowsImage && _bundleId.hasMatch(id),
      TargetPlatform.android => !windowsImage && _packageName.hasMatch(id),
      TargetPlatform.linux => !windowsImage && _desktopId.hasMatch(id),
      TargetPlatform.fuchsia => !windowsImage,
    };
  }

  /// Installed and seen applications by name, leaving out [exclude].
  Future<List<ApplicationChoice>> candidates({
    Iterable<String> exclude = const [],
  }) async {
    final (installed, seen) = await (
      _quietly(_catalog.installed()),
      _withSeen((seen) async => seen),
    ).wait;
    return merge(installed: installed, seen: seen, exclude: exclude);
  }

  /// One application per key; an installed name wins and a seen app keeps
  /// its captured icon.
  @visibleForTesting
  List<ApplicationChoice> merge({
    required List<ApplicationChoice> installed,
    required List<ApplicationChoice> seen,
    Iterable<String> exclude = const [],
  }) {
    final excluded = {for (final id in exclude) key(id)};
    final byKey = <String, ApplicationChoice>{};
    for (final app in installed) {
      byKey.putIfAbsent(key(app.id), () => app);
    }
    for (final app in seen) {
      if (!attributable(app.id)) continue;
      final known = byKey[key(app.id)];
      byKey[key(app.id)] = known == null
          ? app
          : ApplicationChoice(
              id: known.id,
              name: known.name,
              iconId: app.iconId,
            );
    }
    return [
      for (final entry in byKey.entries)
        if (!excluded.contains(entry.key) && entry.value.name.trim().isNotEmpty)
          entry.value,
    ]..sort((a, b) {
      final byName = a.name.toLowerCase().compareTo(b.name.toLowerCase());
      return byName != 0 ? byName : a.id.compareTo(b.id);
    });
  }

  /// The application [id] names, or null when it is neither installed nor
  /// seen here, such as an entry synced from another platform.
  Future<ApplicationChoice?> describe(String id) =>
      _memoized(_described, key(id), () async {
        final (captured, installed) = await (
          _withSeen(
            (seen) async => seen
                .where((app) => attributable(app.id) && key(app.id) == key(id))
                .firstOrNull,
          ),
          _catalog
              .installedById(id)
              .then<ApplicationChoice?>((app) => app, onError: _none),
        ).wait;
        if (installed == null && captured == null) return null;
        return ApplicationChoice(
          id: id,
          name: installed?.name ?? captured!.name,
          iconId: captured?.iconId,
        );
      });

  /// Records a choice made in a picker so it renders without another lookup.
  void remember(ApplicationChoice app) {
    _described[key(app.id)] = _Lookup(Future.value(app));
  }

  /// The captured icon when History has one, else the installed one.
  Future<HistorySourceAppIcon?> icon(ApplicationChoice app) =>
      _memoized(_icons, key(app.id), () async {
        if (app.iconId case final iconId?) {
          final captured = await _catalog
              .seenIcon(iconId)
              .then<HistorySourceAppIcon?>((icon) => icon, onError: _none);
          if (captured != null) return captured;
        }
        return _catalog
            .installedIcon(app.id)
            .then<HistorySourceAppIcon?>((icon) => icon, onError: _none);
      });

  Future<T?> _memoized<T>(
    Map<String, _Lookup<T>> memo,
    String key,
    Future<T?> Function() load,
  ) {
    final known = memo[key];
    final missedAt = known?.missedAt;
    if (known != null &&
        (missedAt == null || _clock().difference(missedAt) < missLifetime)) {
      return known.value;
    }
    final lookup = _Lookup<T>(load());
    memo[key] = lookup;
    lookup.value.then<void>((value) {
      if (value == null) lookup.missedAt = _clock();
    }, onError: (Object _) => lookup.missedAt = _clock());
    return lookup.value;
  }

  /// Runs [use] on the seen applications, loading them once for concurrent
  /// users and releasing them when the last one finishes.
  Future<T> _withSeen<T>(
    Future<T> Function(List<ApplicationChoice>) use,
  ) async {
    _seenUsers++;
    try {
      return await use(await _quietly(_seen ??= _catalog.seen()));
    } finally {
      if (--_seenUsers == 0) _seen = null;
    }
  }

  static Future<List<ApplicationChoice>> _quietly(
    Future<List<ApplicationChoice>> apps,
  ) => apps.then(
    (apps) => apps,
    onError: (Object _) => const <ApplicationChoice>[],
  );

  static Null _none(Object _) => null;

  static final _windowsImage = RegExp(
    r'^[^\\/:\x00-\x1f]+\.exe$',
    caseSensitive: false,
  );
  static final _bundleId = RegExp(r'^[A-Za-z0-9_-]+(\.[A-Za-z0-9_-]+)+$');
  static final _packageName = RegExp(
    r'^[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z][A-Za-z0-9_]*)+$',
  );
  static final _desktopId = RegExp(r'^[A-Za-z0-9._-]+$');
}

class _Lookup<T> {
  _Lookup(this.value);

  final Future<T?> value;
  DateTime? missedAt;
}
