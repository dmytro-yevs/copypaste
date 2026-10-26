import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/services.dart';

import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import '../../devices/flutter_rust_devices_gateway.dart';
import '../../../shared/runtime_event_service.dart';
import '../models/history_models.dart';
import 'history_repository.dart';
import '../controller/history_spoiler_controller.dart';
import 'history_file_importer.dart';

/// Production History port backed only by generated Flutter/Rust bindings.
///
/// Runtime startup and its isolated debug profile are owned by app integration.
/// This adapter never starts a daemon and never supplies a fallback data source.
class RuntimeHistoryRepository
    implements
        HistoryRepository,
        HistorySpoilerRepository,
        HistoryFileDownloadCancellation,
        HistoryFileProgressRepository,
        HistoryDownloadedFileRepository {
  RuntimeHistoryRepository({RuntimeWatchLease? watchLease})
    : _watchLease = watchLease;

  final RuntimeWatchLease? _watchLease;
  bool _disposed = false;
  final _downloadTokens = <String, String>{};

  @override
  Stream<HistoryRuntimeEvent> watch() {
    _ensureActive();
    if (_watchLease == null) {
      return RuntimeEventService.instance.events
          .where((event) => event.kind == 'items' || event.kind == 'ready')
          .map((_) => HistoryRuntimeEvent.itemsChanged);
    }
    return _watch();
  }

  Stream<HistoryRuntimeEvent> _watch() async* {
    final lease = _watchLease!;
    final watchId = await lease.watchId;
    try {
      await for (final event in runtime.watchRuntime(watchId: watchId)) {
        if (event.kind == 'items') yield HistoryRuntimeEvent.itemsChanged;
      }
    } finally {
      await lease.release(watchId);
    }
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
              iconId: facet.iconId,
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
  Future<HistoryImagePreview?> imagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) async {
    _ensureActive();
    return _fromRuntimeImagePreview(
      await runtime.clipImagePreview(
        id: id,
        maxEdge: maxEdge,
        bounds: bounds == null
            ? null
            : runtime.ClipImagePreviewBounds(
                width: bounds.width,
                height: bounds.height,
              ),
      ),
    );
  }

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async {
    _ensureActive();
    final preview = await runtime.sourceAppIcon(id: id);
    if (preview == null) return null;
    return HistorySourceAppIcon(preview.pngBytes);
  }

  @override
  Future<void> copy(String id) async {
    _ensureActive();
    await _fileOperation(() => runtime.copyClip(id: id));
  }

  @override
  Future<void> copyPlainText(String id) async {
    _ensureActive();
    await runtime.copyClipAsPlainText(id: id);
  }

  @override
  Future<void> copyText(String id, String text) async {
    _ensureActive();
    await runtime.copyClipText(id: id, text: text);
  }

  @override
  Future<void> saveFile(String id, String destinationPath) async {
    _ensureActive();
    final random = Random.secure();
    final token = base64UrlEncode(
      List<int>.generate(16, (_) => random.nextInt(256)),
    );
    _downloadTokens[id] = token;
    try {
      await _fileOperation(
        () => runtime.saveClipFile(
          id: id,
          destPath: destinationPath,
          downloadToken: token,
        ),
      );
    } finally {
      if (_downloadTokens[id] == token) _downloadTokens.remove(id);
    }
  }

  @override
  Future<String?> downloadedFile(String id) =>
      runtime.downloadedClipFile(id: id);

  @override
  Future<bool> saveFileWithProgress(
    String id,
    String destinationPath,
    void Function(int written, int total) onProgress,
  ) async {
    _ensureActive();
    final service = RuntimeEventService.instance;
    final token = base64UrlEncode(
      List<int>.generate(16, (_) => Random.secure().nextInt(256)),
    );
    final finished = Completer<void>();
    finished.future.ignore();
    var checkAgain = false;
    var checking = false;
    Future<void> reconcile() async {
      if (finished.isCompleted) return;
      if (checking) {
        checkAgain = true;
        return;
      }
      checking = true;
      try {
        do {
          checkAgain = false;
          final records =
              jsonDecode(await runtime.fileOperations(clear: false, id: token))
                  as List;
          final record = records.cast<Map>().firstOrNull;
          if (record == null) continue;
          onProgress(record['written'] as int, record['total'] as int);
          if (record['phase'] == 'completed') {
            finished.complete();
          } else if (record['phase'] != 'running') {
            finished.completeError(
              HistoryFileOperationException(
                record['error'] as String? ??
                    'The file download was cancelled.',
              ),
            );
          }
        } while (checkAgain && !finished.isCompleted);
      } catch (error, trace) {
        if (!finished.isCompleted) finished.completeError(error, trace);
      } finally {
        checking = false;
      }
    }

    final subscription = service.events.listen(
      (event) {
        final progress = event.fileOperation;
        if (progress?.id == token && progress?.phase == 'running') {
          onProgress(progress!.written.toInt(), progress.total.toInt());
        } else if (event.kind == 'ready' || progress?.id == token) {
          unawaited(reconcile());
        }
      },
      onError: (Object _, StackTrace _) {
        // The runtime owns the operation; reconnect reconciles its durable result.
      },
    );
    _downloadTokens[id] = token;
    try {
      try {
        if (!await service.ready.timeout(const Duration(seconds: 5))) {
          throw const HistoryFileOperationException(
            'The download service is unavailable. Reopen CopyPaste and try again.',
          );
        }
      } on TimeoutException {
        throw const HistoryFileOperationException(
          'The download service did not connect. Try again.',
        );
      }
      await _fileOperation(
        () => runtime.startFileDownload(
          id: id,
          destPath: destinationPath,
          token: token,
        ),
      );
      await reconcile();
      await finished.future;
      return true;
    } finally {
      if (_downloadTokens[id] == token) _downloadTokens.remove(id);
      await subscription.cancel();
    }
  }

  @override
  Future<void> cancelFileDownload(String id) async {
    final token = _downloadTokens[id];
    if (token != null) await runtime.cancelClipFileDownload(id: token);
  }

  Future<T> _fileOperation<T>(Future<T> Function() operation) async {
    try {
      return await operation();
    } on runtime.RuntimeError catch (error) {
      throw HistoryFileOperationException(error.message);
    } on PlatformException catch (error) {
      throw HistoryFileOperationException(
        error.message ?? 'The selected file could not be accessed.',
      );
    }
  }

  @override
  Future<void> importFile(HistoryImportFile file) async {
    _ensureActive();
    await _fileOperation(() async {
      final reference = file.sourceReference;
      if (reference != null) {
        await runtime.addFileReference(
          reference: reference,
          filename: file.name,
          mimeType: file.mimeType,
          sizeHint: file.sizeBytes == null
              ? null
              : BigInt.from(file.sizeBytes!),
        );
        return;
      }
      final maxBytes = (await runtime.historyFileImportMaxBytes()).toInt();
      final path = await file.prepare(maxBytes);
      await runtime.importClipFile(
        path: path,
        filename: file.name,
        mimeType: file.mimeType,
        sourceReference: file.sourceReference,
      );
    });
  }

  @override
  Future<HistorySpoilerState> readSpoilers() async {
    _ensureActive();
    final state = await runtime.getSpoilerState();
    return HistorySpoilerState(
      state.revision.toInt(),
      state.revealedIds.toSet(),
    );
  }

  @override
  Future<HistorySpoilerState> setSpoilerRevealed(
    String? id,
    bool revealed,
  ) async {
    _ensureActive();
    final state = await runtime.setSpoilerRevealed(id: id, revealed: revealed);
    return HistorySpoilerState(
      state.revision.toInt(),
      state.revealedIds.toSet(),
    );
  }

  @override
  Future<HistoryClip> setSecret(String id, bool secret) async {
    _ensureActive();
    return mapDetailClip(await runtime.setClipSecret(id: id, secret: secret));
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
    await _watchLease?.dispose();
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
    final active = _watchId;
    if (active != null) return active;
    final allocated = allocate();
    _watchId = allocated;
    unawaited(_clearFailedAllocation(allocated));
    return allocated;
  }

  Future<void> _clearFailedAllocation(Future<BigInt> allocation) async {
    try {
      await allocation;
    } catch (_) {
      if (identical(_watchId, allocation)) _watchId = null;
    }
  }

  Future<void> release(BigInt watchId) async {
    final active = _watchId;
    if (active == null ||
        await active != watchId ||
        !identical(_watchId, active)) {
      return;
    }
    _watchId = null;
    await cancel(watchId);
  }

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    final watchId = _watchId;
    _watchId = null;
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
    secret: clip.secret,
    transient: clip.transient,
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
    sourceAppIconId: clip.sourceAppIconId,
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
  preview.pngBytes,
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
