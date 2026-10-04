import 'dart:convert';

import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import '../../devices/flutter_rust_devices_gateway.dart';
import '../models/history_models.dart';
import 'history_repository.dart';

/// Production History port backed only by generated Flutter/Rust bindings.
///
/// Runtime startup and its isolated debug profile are owned by app integration.
/// This adapter never starts a daemon and never supplies a fallback data source.
class RuntimeHistoryRepository implements HistoryRepository {
  RuntimeHistoryRepository({RuntimeWatchLease? watchLease})
    : _watchLease =
          watchLease ??
          RuntimeWatchLease(
            allocate: runtime.allocateRuntimeWatch,
            cancel: (watchId) => runtime.cancelRuntimeWatch(watchId: watchId),
          );

  final RuntimeWatchLease _watchLease;
  bool _disposed = false;

  @override
  Stream<HistoryRuntimeEvent> watch() {
    _ensureActive();
    return _watchLease.watchId
        .asStream()
        .asyncExpand((watchId) => runtime.watchRuntime(watchId: watchId))
        .where((event) => event.kind == 'items')
        .map((_) => HistoryRuntimeEvent.itemsChanged);
  }

  @override
  Future<HistoryFacets> facets() async {
    _ensureActive();
    return mapFacets(await runtime.historyFacets());
  }

  /// Maps only Rust-provided labels and opaque IDs for server-side filters.
  static HistoryFacets mapFacets(runtime.HistoryFacets facets) {
    return HistoryFacets(
      originDevices: facets.originDevices
          .map(
            (facet) => HistoryDeviceFacet(
              id: facet.id,
              label: facet.label,
              deviceClass: mapRuntimeDeviceClass(facet.deviceClass),
            ),
          )
          .toList(growable: false),
      sourceApps: facets.sourceApps
          .map(
            (facet) => HistorySourceAppFacet(
              id: facet.id,
              label: facet.label,
              iconItemId: facet.iconItemId,
            ),
          )
          .toList(growable: false),
    );
  }

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    _ensureActive();
    final page = await runtime.queryClips(
      query: runtime.ClipQuery(
        search: query.hasSearch ? query.search.trim() : null,
        contentClasses: _toRuntimeContentClasses(query.kind),
        semanticKinds: _toRuntimeSemanticKinds(query.kind),
        pinnedOnly: query.pinnedOnly,
        originDeviceId: query.origin,
        sourceAppBundleId: query.sourceApp,
        sort: _toRuntimeSort(query.sort),
      ),
      limit: limit,
      cursor: cursor,
    );
    return HistoryClipPage(
      items: page.clips.map(_fromRuntimeListClip).toList(growable: false),
      nextCursor: page.nextCursor,
      skippedUndecryptable: page.skippedUndecryptable,
    );
  }

  @override
  Future<HistoryClip> get(String id) async {
    _ensureActive();
    return _fromRuntimeDetailClip(await runtime.getClip(id: id));
  }

  @override
  Future<HistoryImagePreview?> imagePreview(String id, {int? maxEdge}) async {
    _ensureActive();
    return _fromRuntimeImagePreview(
      await runtime.clipImagePreview(id: id, maxEdge: maxEdge),
    );
  }

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async {
    _ensureActive();
    final preview = await runtime.clipSourceAppIcon(id: id);
    if (preview == null) return null;
    return HistorySourceAppIcon(base64Decode(preview.pngBase64));
  }

  @override
  Future<void> copy(String id) async {
    _ensureActive();
    await runtime.copyClip(id: id);
  }

  @override
  Future<void> copyPlainText(String id) async {
    _ensureActive();
    await runtime.copyClipAsPlainText(id: id);
  }

  @override
  Future<void> saveFile(String id, String destinationPath) async {
    _ensureActive();
    await runtime.saveClipFile(id: id, destPath: destinationPath);
  }

  @override
  Future<void> setPinned(String id, bool pinned) async {
    _ensureActive();
    await runtime.setClipPinned(id: id, pinned: pinned);
  }

  @override
  Future<void> delete(String id) async {
    _ensureActive();
    await runtime.deleteClip(id: id);
  }

  @override
  Future<void> deleteAll() async {
    _ensureActive();
    final ceiling = await runtime.historyCeiling();
    await runtime.deleteClipsThrough(
      through: PlatformInt64Util.from(ceiling.toInt()),
    );
  }

  @override
  Future<void> reorderPinned(List<String> ids) async {
    _ensureActive();
    await runtime.reorderPinnedClips(ids: ids);
  }

  /// Closes the dedicated watch connection. The app integration owns this call
  /// because a runtime repository can be shared by persistent destinations.
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _watchLease.dispose();
  }

  void _ensureActive() {
    if (_disposed) throw StateError('History runtime repository is disposed.');
  }

  /// Maps generated Rust data at the one permitted boundary into feature data.
  static HistoryClip mapListClip(runtime.Clip clip) => _fromRuntimeClip(clip);

  /// Maps the explicit get result and keeps its body out of list responses.
  static HistoryClip mapDetailClip(runtime.Clip clip) =>
      _fromRuntimeClip(clip, includeBody: true);

  static HistoryImagePreview mapImagePreview(
    runtime.ClipImagePreview preview,
  ) => _fromRuntimeImagePreview(preview);
}

/// Owns one generated runtime Watch handle without affecting other features.
class RuntimeWatchLease {
  RuntimeWatchLease({required this.allocate, required this.cancel});

  final Future<BigInt> Function() allocate;
  final Future<void> Function(BigInt watchId) cancel;
  Future<BigInt>? _watchId;
  bool _disposed = false;

  Future<BigInt> get watchId {
    if (_disposed) {
      return Future<BigInt>.error(
        StateError('Runtime Watch lease is disposed.'),
      );
    }
    return _watchId ??= allocate();
  }

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    final watchId = _watchId;
    if (watchId != null) await cancel(await watchId);
  }
}

HistoryClip _fromRuntimeListClip(runtime.Clip clip) =>
    RuntimeHistoryRepository.mapListClip(clip);

HistoryClip _fromRuntimeDetailClip(runtime.Clip clip) =>
    RuntimeHistoryRepository.mapDetailClip(clip);

HistoryClip _fromRuntimeClip(runtime.Clip clip, {bool includeBody = false}) {
  final details = clip.fileDetails;
  final imageDetails = clip.imageDetails;
  return HistoryClip(
    id: clip.id,
    contentType: clip.contentType,
    preview: clip.content,
    body: includeBody ? clip.content : null,
    createdAt: DateTime.fromMillisecondsSinceEpoch(
      clip.createdAtMs.toInt(),
      isUtc: true,
    ),
    pinned: clip.pinned,
    kind: _fromRuntimeKind(clip.contentClass, clip.semanticKind),
    origin: clip.originDeviceName,
    originDeviceClass: mapRuntimeDeviceClass(clip.originDeviceClass),
    sourceApp: clip.sourceAppName,
    truncated: clip.truncated,
    colorRgba: clip.colorRgba,
    tooLargeToSync: clip.tooLargeToSync,
    file: details == null
        ? null
        : HistoryFileDetails(
            name: details.filename,
            mimeType: details.mimeType,
            sourceReference: details.sourceReference,
            sourceAvailable: details.sourceAvailable,
            sizeBytes: details.sizeBytes.toInt(),
            fileCount: details.fileCount,
          ),
    image: imageDetails == null
        ? null
        : HistoryImageDetails(
            width: imageDetails.width,
            height: imageDetails.height,
            sizeBytes: imageDetails.sizeBytes.toInt(),
          ),
  );
}

HistoryImagePreview _fromRuntimeImagePreview(
  runtime.ClipImagePreview preview,
) => HistoryImagePreview(
  base64Decode(preview.pngBase64),
  width: preview.width,
  height: preview.height,
);

HistoryClipKind _fromRuntimeKind(
  runtime.ClipContentClass contentClass,
  runtime.ClipSemanticKind? semanticKind,
) {
  if (contentClass != runtime.ClipContentClass.text) {
    return switch (contentClass) {
      runtime.ClipContentClass.text => HistoryClipKind.text,
      runtime.ClipContentClass.image => HistoryClipKind.image,
      runtime.ClipContentClass.file => HistoryClipKind.file,
      runtime.ClipContentClass.other => HistoryClipKind.other,
    };
  }
  return switch (semanticKind) {
    runtime.ClipSemanticKind.plainText || null => HistoryClipKind.text,
    runtime.ClipSemanticKind.link => HistoryClipKind.link,
    runtime.ClipSemanticKind.email => HistoryClipKind.email,
    runtime.ClipSemanticKind.color => HistoryClipKind.color,
    runtime.ClipSemanticKind.phone => HistoryClipKind.phone,
    runtime.ClipSemanticKind.code => HistoryClipKind.code,
    runtime.ClipSemanticKind.json => HistoryClipKind.json,
    runtime.ClipSemanticKind.path => HistoryClipKind.path,
  };
}

List<runtime.ClipContentClass> _toRuntimeContentClasses(
  HistoryClipKind? kind,
) => switch (kind) {
  HistoryClipKind.image => const [runtime.ClipContentClass.image],
  HistoryClipKind.file => const [runtime.ClipContentClass.file],
  HistoryClipKind.other => const [runtime.ClipContentClass.other],
  null ||
  HistoryClipKind.text ||
  HistoryClipKind.link ||
  HistoryClipKind.email ||
  HistoryClipKind.color ||
  HistoryClipKind.phone ||
  HistoryClipKind.code ||
  HistoryClipKind.json ||
  HistoryClipKind.path => const [],
};

List<runtime.ClipSemanticKind> _toRuntimeSemanticKinds(HistoryClipKind? kind) =>
    switch (kind) {
      HistoryClipKind.text => const [runtime.ClipSemanticKind.plainText],
      HistoryClipKind.link => const [runtime.ClipSemanticKind.link],
      HistoryClipKind.email => const [runtime.ClipSemanticKind.email],
      HistoryClipKind.color => const [runtime.ClipSemanticKind.color],
      HistoryClipKind.phone => const [runtime.ClipSemanticKind.phone],
      HistoryClipKind.code => const [runtime.ClipSemanticKind.code],
      HistoryClipKind.json => const [runtime.ClipSemanticKind.json],
      HistoryClipKind.path => const [runtime.ClipSemanticKind.path],
      null ||
      HistoryClipKind.image ||
      HistoryClipKind.file ||
      HistoryClipKind.other => const [],
    };

runtime.ClipSort _toRuntimeSort(HistorySort sort) => switch (sort) {
  HistorySort.newest => runtime.ClipSort.newest,
  HistorySort.oldest => runtime.ClipSort.oldest,
  HistorySort.relevance => runtime.ClipSort.relevance,
};
