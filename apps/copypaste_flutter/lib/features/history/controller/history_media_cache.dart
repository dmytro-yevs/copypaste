import 'dart:async';
import 'dart:collection';

import 'package:flutter/foundation.dart';

import '../models/history_models.dart';
import 'history_resource_limits.dart';
import 'history_scope.dart';
import 'history_summary.dart';

typedef _ImagePreviewKey = ({String id, int? edge, int? width, int? height});

/// Bounded image-preview, source-icon and secret-content caches for rows.
class HistoryMediaCache {
  HistoryMediaCache(
    this._scope, {
    required int Function() queryEpoch,
    required Set<String> Function() retainedSourceIconIds,
  }) : _queryEpoch = queryEpoch,
       _retainedSourceIconIds = retainedSourceIconIds;

  static const int _maxCachedMedia = 48;

  final HistoryScope _scope;
  final int Function() _queryEpoch;
  final Set<String> Function() _retainedSourceIconIds;
  final LinkedHashMap<String, Future<HistoryClip>> _spoilerContent =
      LinkedHashMap();
  final LinkedHashMap<_ImagePreviewKey, HistoryImagePreview?> _imagePreviews =
      LinkedHashMap();
  final LinkedHashMap<String, HistorySourceAppIcon?> _sourceIcons =
      LinkedHashMap();
  final Map<_ImagePreviewKey, Future<HistoryImagePreview?>> _imagePreviewLoads =
      {};
  final Map<String, Future<HistorySourceAppIcon?>> _sourceIconLoads = {};

  /// Loads only mounted secret content for layout beneath a nonpainting mask.
  Future<HistoryClip> spoilerContent(String id) {
    final cached = _spoilerContent.remove(id);
    final result = cached ?? _scope.repository.get(id).then(historySummary);
    _spoilerContent[id] = result;
    while (_spoilerContent.length > _maxCachedMedia) {
      _spoilerContent.remove(_spoilerContent.keys.first);
    }
    return result;
  }

  void forgetSpoilerContent(String id) => _spoilerContent.remove(id);

  Future<HistoryImagePreview?> requestImagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) {
    final cacheKey = (
      id: id,
      edge: maxEdge,
      width: bounds?.width,
      height: bounds?.height,
    );
    if (!_scope.disposed && !_scope.suspended && bounds != null) {
      final edge = (maxEdge ?? 2048).clamp(1, 2048);
      final width = bounds.width.clamp(1, edge);
      final height = bounds.height.clamp(1, edge);
      // Resizing the inspector can change the row width while height remains
      // the limiting dimension. Reuse the exact pixels in that case, avoiding
      // another native decode and a second engine image with identical content.
      // A smaller preview must never satisfy a request for higher resolution.
      for (final entry in _imagePreviews.entries) {
        final preview = entry.value;
        if (entry.key.id == id &&
            preview != null &&
            preview.width > 0 &&
            preview.height > 0 &&
            preview.width <= width &&
            preview.height <= height &&
            (preview.width == width || preview.height == height)) {
          _imagePreviews.remove(entry.key);
          _imagePreviews[entry.key] = preview;
          return SynchronousFuture(preview);
        }
      }
    }
    return _loadCached(
      cache: _imagePreviews,
      inFlight: _imagePreviewLoads,
      id: cacheKey,
      loader: () =>
          _scope.repository.imagePreview(id, maxEdge: maxEdge, bounds: bounds),
    );
  }

  Future<HistorySourceAppIcon?> requestSourceIcon(String? iconId) {
    if (iconId == null) return SynchronousFuture(null);
    return _loadCached(
      cache: _sourceIcons,
      inFlight: _sourceIconLoads,
      id: iconId,
      loader: () => _scope.repository.sourceAppIcon(iconId),
      retainedIds: _retainedSourceIconIds,
      queryBound: false,
    );
  }

  void trimSourceIcons() =>
      _trimCachedMedia(_sourceIcons, retainedIds: _retainedSourceIconIds());

  /// Releases every retained payload; loads started earlier cannot refill it.
  void clear() {
    _spoilerContent.clear();
    _imagePreviews.clear();
    _sourceIcons.clear();
    _imagePreviewLoads.clear();
    _sourceIconLoads.clear();
  }

  void dispose() {
    _spoilerContent.clear();
    _imagePreviewLoads.clear();
    _sourceIconLoads.clear();
  }

  void _trimCachedMedia<K, T>(
    LinkedHashMap<K, T?> cache, {
    Set<K> retainedIds = const {},
  }) {
    final maxBytes = _mediaCacheLimit(cache);
    var bytes = cache.values.fold<int>(
      0,
      (sum, value) => sum + _mediaBytes(value),
    );
    // Prefer retiring invisible entries, but visibility cannot bypass the byte
    // ceiling. Mounted images retain their own reference until disposed.
    final candidates = [
      ...cache.keys.where((id) => !retainedIds.contains(id)),
      ...cache.keys.where(retainedIds.contains),
    ];
    for (final id in candidates) {
      final overBytes = bytes > maxBytes;
      final overCount =
          cache.length > HistoryResourceLimits.retainedSourceIcons ||
          (cache.length > _maxCachedMedia && !retainedIds.contains(id));
      if (!overBytes && !overCount) continue;
      bytes -= _mediaBytes(cache.remove(id));
    }
  }

  int _mediaCacheLimit(Object cache) => identical(cache, _sourceIcons)
      ? HistoryResourceLimits.sourceIconCacheBytes
      : HistoryResourceLimits.previewCacheBytes;

  int _mediaBytes(Object? value) => switch (value) {
    HistoryImagePreview() => value.bytes.buffer.lengthInBytes,
    HistorySourceAppIcon() => value.bytes.buffer.lengthInBytes,
    _ => 0,
  };

  Future<T?> _loadCached<K, T>({
    required LinkedHashMap<K, T?> cache,
    required Map<K, Future<T?>> inFlight,
    required K id,
    required Future<T?> Function() loader,
    Set<K> Function()? retainedIds,
    bool queryBound = true,
  }) {
    if (_scope.disposed || _scope.suspended) {
      return SynchronousFuture<T?>(null);
    }
    final hasCached = cache.containsKey(id);
    final cached = cache.remove(id);
    if (hasCached) {
      cache[id] = cached;
      return SynchronousFuture<T?>(cached);
    }
    final existing = inFlight[id];
    if (existing != null) return existing;
    final epoch = _queryEpoch();
    final generation = _scope.generation;
    bool current() =>
        !_scope.disposed &&
        !_scope.suspended &&
        generation == _scope.generation &&
        (!queryBound || epoch == _queryEpoch());
    late final Future<T?> request;
    request = () async {
      try {
        final value = await loader();
        if (!current()) return null;
        // An oversized preview may still be displayed by its caller. It must
        // not evict every reusable thumbnail only to be evicted itself.
        if (_mediaBytes(value) <= _mediaCacheLimit(cache)) {
          cache[id] = value;
          _trimCachedMedia(cache, retainedIds: retainedIds?.call() ?? const {});
        }
        return value;
      } catch (_) {
        if (current()) {
          cache[id] = null;
          _trimCachedMedia(cache, retainedIds: retainedIds?.call() ?? const {});
        }
        return null;
      } finally {
        if (generation == _scope.generation) inFlight.remove(id);
      }
    }();
    inFlight[id] = request;
    return request;
  }
}
