import 'package:flutter/foundation.dart';

import '../../history/models/history_models.dart';
import '../models/application_models.dart';
import '../repository/application_catalog.dart';

/// Application names and icons for one view: installed apps merged with apps
/// seen in History. Lookups are memoized only until the owning view drops
/// this directory.
class ApplicationDirectory {
  ApplicationDirectory(this._catalog, {TargetPlatform? platform})
    : _platform = platform ?? defaultTargetPlatform;

  final ApplicationCatalog _catalog;
  final TargetPlatform _platform;
  Future<List<ApplicationChoice>>? _seen;
  final _described = <String, Future<ApplicationChoice?>>{};
  final _icons = <String, Future<HistorySourceAppIcon?>>{};

  /// The key every spelling of one application's identifier shares. Windows
  /// image names compare without case or `.exe`, as capture exclusions do.
  String key(String id) {
    if (_platform != TargetPlatform.windows) return id;
    final lower = id.trim().toLowerCase();
    return lower.endsWith('.exe')
        ? lower.substring(0, lower.length - '.exe'.length)
        : lower;
  }

  /// Installed and seen applications by name, leaving out [exclude].
  Future<List<ApplicationChoice>> candidates({
    Iterable<String> exclude = const [],
  }) async {
    final (installed, seen) = await (
      _quietly(_catalog.installed()),
      _quietly(_seenApps()),
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
  Future<ApplicationChoice?> describe(
    String id,
  ) => _described.putIfAbsent(key(id), () async {
    final (seen, installed) = await (
      _quietly(_seenApps()),
      _catalog
          .installedById(id)
          .then<ApplicationChoice?>((app) => app, onError: (Object _) => null),
    ).wait;
    final captured = seen.where((app) => key(app.id) == key(id)).firstOrNull;
    if (installed == null && captured == null) return null;
    return ApplicationChoice(
      id: id,
      name: installed?.name ?? captured!.name,
      iconId: captured?.iconId,
    );
  });

  /// Records a choice made in a picker so it renders without another lookup.
  void remember(ApplicationChoice app) {
    _described[key(app.id)] = Future.value(app);
  }

  /// The captured icon when History has one, else the installed one.
  Future<HistorySourceAppIcon?> icon(ApplicationChoice app) =>
      _icons.putIfAbsent(key(app.id), () async {
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

  Future<List<ApplicationChoice>> _seenApps() => _seen ??= _catalog.seen();

  static Future<List<ApplicationChoice>> _quietly(
    Future<List<ApplicationChoice>> apps,
  ) => apps.then(
    (apps) => apps,
    onError: (Object _) => const <ApplicationChoice>[],
  );

  static Null _none(Object _) => null;
}
