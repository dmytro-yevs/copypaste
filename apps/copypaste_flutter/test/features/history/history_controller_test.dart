import 'dart:async';

import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_downloader.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('HistoryController', () {
    test('classifies every supported clip representation', () {
      expect(HistoryClipKindX.fromContentType('text'), HistoryClipKind.text);
      expect(
        HistoryClipKindX.fromContentType('text/html'),
        HistoryClipKind.text,
      );
      expect(
        HistoryClipKindX.fromContentType('image/tiff'),
        HistoryClipKind.image,
      );
      expect(HistoryClipKindX.fromContentType('file'), HistoryClipKind.file);
      expect(
        HistoryClipKindX.fromContentType('application/octet-stream'),
        HistoryClipKind.other,
      );
    });

    test('exposes every semantic kind as a distinct filter label', () {
      expect(HistoryClipKind.values.map((kind) => kind.label), [
        'Text',
        'Link',
        'Email',
        'Color',
        'Phone',
        'Code',
        'JSON',
        'Path',
        'Image',
        'File',
        'Other',
      ]);
      expect(HistoryClipKind.values.where((kind) => kind.isTextual), [
        HistoryClipKind.text,
        HistoryClipKind.link,
        HistoryClipKind.email,
        HistoryClipKind.color,
        HistoryClipKind.phone,
        HistoryClipKind.code,
        HistoryClipKind.json,
        HistoryClipKind.path,
      ]);
    });

    test('ignores a stale query response after the query changes', () async {
      final repository = _HistoryRepository();
      final first = Completer<HistoryClipPage>();
      final second = Completer<HistoryClipPage>();
      repository.pages.addAll([first.future, second.future]);
      final controller = HistoryController(
        repository,
        searchDebounce: Duration.zero,
      );

      unawaited(controller.initialize());
      await Future<void>.delayed(Duration.zero);
      unawaited(controller.updateQuery(const HistoryQuery(search: 'second')));
      await Future<void>.delayed(Duration.zero);

      second.complete(HistoryClipPage(items: [_clip('current')]));
      await Future<void>.delayed(Duration.zero);
      first.complete(HistoryClipPage(items: [_clip('stale')]));
      await Future<void>.delayed(Duration.zero);

      expect(controller.items.single.id, 'current');
      expect(controller.query.search, 'second');
      controller.dispose();
    });

    test(
      'uses the opaque cursor only for the unchanged active query',
      () async {
        final repository = _HistoryRepository();
        repository.pages.addAll([
          Future.value(
            HistoryClipPage(items: [_clip('one')], nextCursor: 'opaque-cursor'),
          ),
          Future.value(HistoryClipPage(items: [_clip('two')])),
        ]);
        final controller = HistoryController(repository);

        await controller.initialize();
        await controller.loadMore();

        expect(repository.requests, hasLength(2));
        expect(repository.requests.last.cursor, 'opaque-cursor');
        expect(repository.requests.last.query.search, isEmpty);
        expect(controller.items.map((item) => item.id), ['one', 'two']);
        controller.dispose();
      },
    );

    test('unblocks later paging after a stale page request finishes', () async {
      final repository = _HistoryRepository();
      final stalePage = Completer<HistoryClipPage>();
      repository.pages.addAll([
        Future.value(
          HistoryClipPage(items: [_clip('old')], nextCursor: 'old-cursor'),
        ),
        stalePage.future,
        Future.value(
          HistoryClipPage(items: [_clip('new')], nextCursor: 'new-cursor'),
        ),
        Future.value(HistoryClipPage(items: [_clip('newer')])),
      ]);
      final controller = HistoryController(repository);
      await controller.initialize();

      unawaited(controller.loadMore());
      await Future<void>.delayed(Duration.zero);
      await controller.updateQuery(const HistoryQuery(search: 'new'));
      expect(controller.isLoadingMore, isFalse);

      await controller.loadMore();
      stalePage.complete(HistoryClipPage(items: [_clip('stale')]));
      await Future<void>.delayed(Duration.zero);

      expect(repository.requests.last.cursor, 'new-cursor');
      expect(controller.items.map((item) => item.id), ['new', 'newer']);
      controller.dispose();
    });

    test('refreshes after an items invalidation event', () async {
      final repository = _HistoryRepository();
      repository.pages.addAll([
        Future.value(HistoryClipPage(items: [_clip('before')])),
        Future.value(HistoryClipPage(items: [_clip('after')])),
      ]);
      final controller = HistoryController(repository);

      await controller.initialize();
      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.items.single.id, 'after');
      controller.dispose();
    });

    test('retains skipped-row state when the returned page is empty', () async {
      final repository = _HistoryRepository()
        ..pages.add(
          Future.value(
            const HistoryClipPage(items: [], skippedUndecryptable: 2),
          ),
        );
      final controller = HistoryController(repository);

      await controller.initialize();

      expect(controller.state, HistoryLoadState.empty);
      expect(controller.skippedUndecryptable, 2);
      controller.dispose();
    });

    test(
      'deduplicates pending media requests and keeps detail previews distinct',
      () async {
        final repository = _HistoryRepository();
        final thumbnail = Completer<HistoryImagePreview?>();
        final detail = Completer<HistoryImagePreview?>();
        final sourceIcon = Completer<HistorySourceAppIcon?>();
        repository.imagePreviewFutures.addAll([
          thumbnail.future,
          detail.future,
        ]);
        repository.sourceIconFuture = sourceIcon.future;
        repository.pages.add(
          Future.value(HistoryClipPage(items: [_clip('one')])),
        );
        final controller = HistoryController(repository);
        await controller.initialize();

        final firstThumbnail = controller.requestImagePreview(
          'one',
          maxEdge: 96,
        );
        final repeatedThumbnail = controller.requestImagePreview(
          'one',
          maxEdge: 96,
        );
        final detailPreview = controller.requestImagePreview(
          'one',
          maxEdge: 1024,
        );
        final firstSourceIcon = controller.requestSourceIcon('one');
        final repeatedSourceIcon = controller.requestSourceIcon('one');

        expect(identical(firstThumbnail, repeatedThumbnail), isTrue);
        expect(identical(firstSourceIcon, repeatedSourceIcon), isTrue);
        expect(repository.imagePreviewEdges, [96, 1024]);
        expect(repository.sourceIconCalls, 1);

        thumbnail.complete(null);
        detail.complete(null);
        sourceIcon.complete(null);
        await Future.wait([firstThumbnail, detailPreview, firstSourceIcon]);
        controller.dispose();
      },
    );

    test(
      'loads backend filter labels while retaining their stable IDs',
      () async {
        final repository = _HistoryRepository()
          ..availableFacets = const HistoryFacets(
            originDevices: [
              HistoryDeviceFacet(
                id: 'device-1',
                label: 'Work Mac',
                deviceClass: DeviceClass.laptop,
              ),
            ],
            sourceApps: [
              HistorySourceAppFacet(id: 'com.example.editor', label: 'Editor'),
            ],
          );
        repository.pages.addAll([
          Future.value(const HistoryClipPage(items: [])),
          Future.value(const HistoryClipPage(items: [])),
          Future.value(const HistoryClipPage(items: [])),
        ]);
        final controller = HistoryController(repository);

        await controller.initialize();

        expect(controller.facets.originDevices.single.label, 'Work Mac');
        expect(controller.facets.originDevices.single.id, 'device-1');
        expect(controller.facets.sourceApps.single.label, 'Editor');
        await controller.updateQuery(
          const HistoryQuery(
            origin: 'device-1',
            sourceApp: 'com.example.editor',
          ),
        );
        expect(repository.requests.last.query.origin, 'device-1');
        expect(repository.requests.last.query.sourceApp, 'com.example.editor');
        await controller.updateQuery(
          controller.query.copyWith(clearOrigin: true, clearSourceApp: true),
        );
        expect(repository.requests.last.query.origin, isNull);
        expect(repository.requests.last.query.sourceApp, isNull);
        controller.dispose();
      },
    );

    test('restores a pin when the action fails', () async {
      final repository = _HistoryRepository()
        ..setPinnedError = StateError('offline');
      repository.pages.add(
        Future.value(HistoryClipPage(items: [_clip('one')])),
      );
      final controller = HistoryController(repository);
      await controller.initialize();

      await controller.togglePin(controller.items.single);

      expect(controller.items.single.pinned, isFalse);
      expect(controller.errorMessage, 'The pin could not be updated.');
      controller.dispose();
    });

    test('guards an in-flight pin mutation from a second invocation', () async {
      final repository = _HistoryRepository();
      final pending = Completer<void>();
      repository.setPinnedPending = pending.future;
      repository.pages.add(
        Future.value(HistoryClipPage(items: [_clip('one')])),
      );
      final controller = HistoryController(repository);
      await controller.initialize();

      final first = controller.togglePin(controller.items.single);
      final second = await controller.togglePin(controller.items.single);
      expect(repository.setPinnedCalls, 1);
      expect(second, isFalse);
      expect(controller.isPinPending('one'), isTrue);

      pending.complete();
      expect(await first, isTrue);
      expect(controller.isPinPending('one'), isFalse);
      controller.dispose();
    });

    test('guards delete mutations and preserves state after failure', () async {
      final repository = _HistoryRepository();
      final pendingDelete = Completer<void>();
      repository.deletePending = pendingDelete.future;
      repository.pages.add(
        Future.value(HistoryClipPage(items: [_clip('one')])),
      );
      final controller = HistoryController(repository);
      await controller.initialize();
      await controller.select('one');

      final firstDelete = controller.deleteSelected();
      final secondDelete = await controller.deleteSelected();
      expect(repository.deleteCalls, 1);
      expect(secondDelete, isFalse);
      pendingDelete.completeError(StateError('offline'));
      expect(await firstDelete, isFalse);
      expect(controller.items.single.id, 'one');

      final pendingAll = Completer<void>();
      repository.deleteAllPending = pendingAll.future;
      final firstAll = controller.deleteAll();
      final secondAll = await controller.deleteAll();
      expect(repository.deleteAllCalls, 1);
      expect(secondAll, isFalse);
      pendingAll.completeError(StateError('offline'));
      expect(await firstAll, isFalse);
      expect(controller.items.single.id, 'one');
      controller.dispose();
    });

    test(
      'downloads stored file bytes when the synced source is unavailable',
      () async {
        final file = HistoryClip(
          id: 'file',
          contentType: 'file',
          preview: '/Users/person/Documents/report.pdf',
          createdAt: DateTime.utc(2026),
          pinned: false,
          file: const HistoryFileDetails(
            name: 'report.pdf',
            mimeType: 'application/pdf',
            sourceReference: '/Users/person/Documents/report.pdf',
            sourceAvailable: false,
            sizeBytes: 42,
            fileCount: 1,
          ),
        );
        final repository = _HistoryRepository()
          ..detailClip = file
          ..pages.add(Future.value(HistoryClipPage(items: [file])));
        final downloader = _HistoryFileDownloader('/tmp/report.pdf');
        final controller = HistoryController(
          repository,
          fileDownloader: downloader,
        );
        await controller.initialize();
        await controller.select(file.id);

        expect(controller.canDownloadSelected, isTrue);
        expect(
          await controller.downloadSelected(),
          HistoryFileDownloadResult.saved,
        );
        expect(repository.savedFiles, [('file', '/tmp/report.pdf')]);
        expect(downloader.presentedPaths, ['/tmp/report.pdf']);
        controller.dispose();
      },
    );
  });
}

HistoryClip _clip(String id) => HistoryClip(
  id: id,
  contentType: 'text/plain',
  preview: id,
  createdAt: DateTime.utc(2026),
  pinned: false,
);

class _QueryRequest {
  const _QueryRequest(this.query, this.cursor);

  final HistoryQuery query;
  final String? cursor;
}

class _HistoryRepository implements HistoryRepository {
  final List<Future<HistoryClipPage>> pages = [];
  final List<_QueryRequest> requests = [];
  final StreamController<HistoryRuntimeEvent> events =
      StreamController.broadcast();
  Object? setPinnedError;
  Future<void>? setPinnedPending;
  Future<void>? deletePending;
  Future<void>? deleteAllPending;
  int setPinnedCalls = 0;
  int deleteCalls = 0;
  int deleteAllCalls = 0;
  HistoryFacets availableFacets = const HistoryFacets();
  final List<Future<HistoryImagePreview?>> imagePreviewFutures = [];
  final List<int?> imagePreviewEdges = [];
  Future<HistorySourceAppIcon?>? sourceIconFuture;
  int sourceIconCalls = 0;
  HistoryClip? detailClip;
  final List<(String, String)> savedFiles = [];

  @override
  Stream<HistoryRuntimeEvent> watch() => events.stream;

  @override
  Future<HistoryFacets> facets() async => availableFacets;

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) {
    requests.add(_QueryRequest(query, cursor));
    return pages.removeAt(0);
  }

  @override
  Future<HistoryClip> get(String id) async => detailClip ?? _clip(id);

  @override
  Future<HistoryImagePreview?> imagePreview(String id, {int? maxEdge}) {
    imagePreviewEdges.add(maxEdge);
    if (imagePreviewFutures.isEmpty) return Future.value(null);
    return imagePreviewFutures.removeAt(0);
  }

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) {
    sourceIconCalls++;
    return sourceIconFuture ?? Future.value(null);
  }

  @override
  Future<void> copy(String id) async {}

  @override
  Future<void> copyPlainText(String id) async {}

  @override
  Future<void> saveFile(String id, String destinationPath) async {
    savedFiles.add((id, destinationPath));
  }

  @override
  Future<void> delete(String id) async {
    deleteCalls++;
    await deletePending;
  }

  @override
  Future<void> deleteAll() async {
    deleteAllCalls++;
    await deleteAllPending;
  }

  @override
  Future<void> reorderPinned(List<String> ids) async {}

  @override
  Future<void> setPinned(String id, bool pinned) async {
    setPinnedCalls++;
    await setPinnedPending;
    if (setPinnedError != null) throw setPinnedError!;
  }
}

class _HistoryFileDownloader implements HistoryFileDownloader {
  _HistoryFileDownloader(this.destination);

  final String? destination;
  final List<String> presentedPaths = [];

  @override
  Future<String?> chooseDestination(HistoryFileDetails file) async =>
      destination;

  @override
  Future<void> presentSavedFile(String path, HistoryFileDetails file) async {
    presentedPaths.add(path);
  }
}
