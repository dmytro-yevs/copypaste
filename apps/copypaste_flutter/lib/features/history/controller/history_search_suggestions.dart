import 'dart:async';

import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:flutter/foundation.dart';

import '../models/history_models.dart';
import '../repository/recent_searches_store.dart';

enum HistorySearchSuggestionKind { recent, tag, sourceApp, clipKind, device }

/// One completion offered under the History search field.
@immutable
class HistorySearchSuggestion {
  const HistorySearchSuggestion._(
    this.kind,
    this.label, {
    this.id,
    this.clipKind,
    this.iconId,
    this.deviceClass = DeviceClass.unknown,
  });

  factory HistorySearchSuggestion.recent(String search) =>
      HistorySearchSuggestion._(HistorySearchSuggestionKind.recent, search);

  factory HistorySearchSuggestion.tag(HistoryTag tag) =>
      HistorySearchSuggestion._(
        HistorySearchSuggestionKind.tag,
        tag.name,
        id: tag.id,
      );

  factory HistorySearchSuggestion.sourceApp(HistorySourceAppFacet app) =>
      HistorySearchSuggestion._(
        HistorySearchSuggestionKind.sourceApp,
        app.label,
        id: app.id,
        iconId: app.iconId,
      );

  factory HistorySearchSuggestion.clipKind(HistoryClipKind kind) =>
      HistorySearchSuggestion._(
        HistorySearchSuggestionKind.clipKind,
        kind.label,
        clipKind: kind,
      );

  factory HistorySearchSuggestion.device(HistoryDeviceFacet device) =>
      HistorySearchSuggestion._(
        HistorySearchSuggestionKind.device,
        device.label,
        id: device.id,
        deviceClass: device.deviceClass,
      );

  final HistorySearchSuggestionKind kind;
  final String label;

  /// The facet id of a tag, source app or device; never rendered.
  final String? id;
  final HistoryClipKind? clipKind;
  final String? iconId;
  final DeviceClass deviceClass;

  @override
  bool operator ==(Object other) =>
      other is HistorySearchSuggestion &&
      other.kind == kind &&
      other.label == label &&
      other.id == id &&
      other.clipKind == clipKind;

  @override
  int get hashCode => Object.hash(kind, label, id, clipKind);
}

/// The search text and query that accepting a suggestion produces.
typedef HistorySearchCompletion = ({String search, HistoryQuery query});

/// Derives suggestions from already-loaded facets and a bounded list of
/// device-local recent searches, and owns the keyboard highlight.
class HistorySearchSuggestionsController extends ChangeNotifier {
  HistorySearchSuggestionsController(this._store);

  static const maxRecentSearches = 10;
  static const maxSuggestions = 8;
  static const _maxRecentShown = 5;
  static const _maxPerFacet = 3;
  static const _maxSearchLength = 200;

  final RecentSearchesStore _store;
  List<String> _recent = const [];
  Future<void>? _loading;
  List<HistorySearchSuggestion> _suggestions = const [];
  int _highlighted = -1;
  bool _disposed = false;

  List<String> get recentSearches => _recent;

  List<HistorySearchSuggestion> get suggestions => _suggestions;

  /// The keyboard-highlighted row, or -1 before any arrow key.
  int get highlighted => _highlighted;

  HistorySearchSuggestion? get highlightedSuggestion =>
      _highlighted < 0 ? null : _suggestions[_highlighted];

  /// Reads the stored recent searches once per controller.
  Future<void> load() => _loading ??= _store.read().then((value) {
    if (_disposed) return;
    _recent = List.unmodifiable(value.take(maxRecentSearches));
  });

  void update(String text, HistoryQuery query, HistoryFacets facets) {
    if (_disposed) return;
    final next = _derive(text, query, facets);
    if (listEquals(next, _suggestions)) return;
    _suggestions = List.unmodifiable(next);
    _highlighted = -1;
    notifyListeners();
  }

  void move(int delta) {
    if (_disposed || _suggestions.isEmpty) return;
    final length = _suggestions.length;
    final start = _highlighted < 0 && delta < 0 ? 0 : _highlighted;
    _highlighted = (start + delta) % length;
    if (_highlighted < 0) _highlighted += length;
    notifyListeners();
  }

  void dismiss() {
    if (_disposed || _suggestions.isEmpty) return;
    _suggestions = const [];
    _highlighted = -1;
    notifyListeners();
  }

  /// Applies [suggestion]: a recent search replaces the text, and a facet
  /// replaces the word being typed with the matching filter.
  HistorySearchCompletion complete(
    HistorySearchSuggestion suggestion,
    String text,
    HistoryQuery query,
  ) {
    dismiss();
    if (suggestion.kind == HistorySearchSuggestionKind.recent) {
      unawaited(remember(suggestion.label));
      return (
        search: suggestion.label,
        query: query.copyWith(
          search: suggestion.label,
          sort: query.hasSearch ? query.sort : HistorySort.relevance,
        ),
      );
    }
    final search = text.substring(0, _tokenStart(text)).trimRight();
    final base = query.copyWith(search: search);
    return (
      search: search,
      query: switch (suggestion.kind) {
        HistorySearchSuggestionKind.tag => base.copyWith(
          tagIds: [...query.tagIds, suggestion.id!],
          untaggedOnly: false,
        ),
        HistorySearchSuggestionKind.sourceApp => base.copyWith(
          sourceApp: suggestion.id,
        ),
        HistorySearchSuggestionKind.clipKind => base.copyWith(
          kind: suggestion.clipKind,
        ),
        HistorySearchSuggestionKind.device => base.copyWith(
          origin: suggestion.id,
        ),
        HistorySearchSuggestionKind.recent => base,
      },
    );
  }

  Future<void> remember(String text) async {
    final search = text.trim();
    if (_disposed || search.isEmpty || search.length > _maxSearchLength) {
      return;
    }
    await load();
    if (_disposed) return;
    final key = search.toLowerCase();
    final next = [
      search,
      ..._recent.where((entry) => entry.toLowerCase() != key),
    ].take(maxRecentSearches).toList(growable: false);
    if (listEquals(next, _recent)) return;
    _recent = List.unmodifiable(next);
    await _store.write(_recent);
  }

  Future<void> clearRecent() async {
    if (_disposed) return;
    _recent = const [];
    final remaining = [
      for (final suggestion in _suggestions)
        if (suggestion.kind != HistorySearchSuggestionKind.recent) suggestion,
    ];
    if (remaining.length != _suggestions.length) {
      _suggestions = List.unmodifiable(remaining);
      _highlighted = -1;
      notifyListeners();
    }
    await _store.write(const []);
  }

  List<HistorySearchSuggestion> _derive(
    String text,
    HistoryQuery query,
    HistoryFacets facets,
  ) {
    final result = <HistorySearchSuggestion>[];
    final whole = text.trim().toLowerCase();
    for (final recent in _recent) {
      final key = recent.toLowerCase();
      if (whole.isNotEmpty && (key == whole || !key.startsWith(whole))) {
        continue;
      }
      result.add(HistorySearchSuggestion.recent(recent));
      if (result.length == (whole.isEmpty ? _maxRecentShown : 2)) break;
    }
    final token = text.substring(_tokenStart(text)).toLowerCase();
    if (token.isEmpty) return result;

    void addMatches<T>(
      Iterable<T> values,
      String Function(T) label,
      bool Function(T) active,
      HistorySearchSuggestion Function(T) create,
    ) {
      var added = 0;
      for (final value in values) {
        if (added == _maxPerFacet || result.length == maxSuggestions) return;
        if (active(value) || !_matches(label(value), token)) continue;
        result.add(create(value));
        added++;
      }
    }

    addMatches<HistoryTag>(
      facets.tags,
      (tag) => tag.name,
      (tag) => query.tagIds.contains(tag.id),
      HistorySearchSuggestion.tag,
    );
    addMatches<HistorySourceAppFacet>(
      facets.sourceApps,
      (app) => app.label,
      (app) => app.id == query.sourceApp,
      HistorySearchSuggestion.sourceApp,
    );
    addMatches<HistoryClipKind>(
      HistoryClipKind.values,
      (kind) => kind.label,
      (kind) => kind == query.kind,
      HistorySearchSuggestion.clipKind,
    );
    addMatches<HistoryDeviceFacet>(
      facets.originDevices,
      (device) => device.label,
      (device) => device.id == query.origin,
      HistorySearchSuggestion.device,
    );
    return result;
  }

  /// Where the word being typed starts; the text length after a space.
  static int _tokenStart(String text) {
    var index = text.length;
    while (index > 0 && text[index - 1].trim().isNotEmpty) {
      index--;
    }
    return index;
  }

  /// Matches the start of the label or of any word in it.
  static bool _matches(String label, String token) {
    final value = label.toLowerCase();
    if (value.startsWith(token)) return true;
    for (var i = value.indexOf(' '); i >= 0; i = value.indexOf(' ', i + 1)) {
      if (value.startsWith(token, i + 1)) return true;
    }
    return false;
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
