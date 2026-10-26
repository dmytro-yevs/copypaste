import 'dart:async';
import 'dart:typed_data';

import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_downloader.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('HistoryController', () {
    test('reuses immutable row snapshots until rows change', () async {
      final repository = _HistoryRepository()
        ..pages.addAll([
          Future.value(
            HistoryClipPage(items: [_clip('one')], nextCursor: 'next'),
          ),
          Future.value(HistoryClipPage(items: [_clip('two')])),
          Future.value(HistoryClipPage(items: [_clip('fresh')])),
          Future.value(HistoryClipPage(items: [_clip('reloaded')])),
        ]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();

      final first = controller.items;
      expect(controller.items, same(first));
      expect(() => first.clear(), throwsUnsupportedError);
      await controller.loadMore();
      final loaded = controller.items;
      expect(loaded, isNot(same(first)));
      expect(first.map((clip) => clip.id), ['one']);
      expect(loaded.map((clip) => clip.id), ['one', 'two']);
      expect(controller.items, same(loaded));

      expect(await controller.togglePin(loaded.first), isTrue);
      final pinned = controller.items;
      expect(pinned, isNot(same(loaded)));
      expect(loaded.first.pinned, isFalse);
      expect(pinned.first.pinned, isTrue);
      expect(await controller.deleteClip('two'), isTrue);
      final deleted = controller.items;
      expect(deleted, isNot(same(pinned)));
      expect(pinned.map((clip) => clip.id), ['one', 'two']);
      expect(deleted.map((clip) => clip.id), ['one']);

      controller.suspend();
      final suspended = controller.items;
      expect(suspended, isEmpty);
      expect(suspended, isNot(same(deleted)));
      expect(deleted.single.id, 'one');
      expect(controller.items, same(suspended));
      await controller.initialize();
      expect(controller.items.single.id, 'fresh');
      expect(controller.items, isNot(same(suspended)));
      expect(suspended, isEmpty);
      final resumed = controller.items;
      await controller.reload();
      expect(controller.items.single.id, 'reloaded');
      expect(resumed.single.id, 'fresh');
      expect(controller.items, isNot(same(resumed)));
    });

    test(
      'reuses identical bounded pixels without reducing preview quality',
      () async {
        final repository = _HistoryRepository();
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        final preview = HistoryImagePreview(
          Uint8List(32),
          width: 568,
          height: 320,
        );
        repository.imagePreviewFutures.add(Future.value(preview));
        expect(
          await controller.requestImagePreview(
            'one',
            bounds: const HistoryImagePreviewBounds(width: 1400, height: 320),
          ),
          same(preview),
        );
        expect(
          await controller.requestImagePreview(
            'one',
            bounds: const HistoryImagePreviewBounds(width: 900, height: 320),
          ),
          same(preview),
        );
        expect(repository.imagePreviewEdges, hasLength(1));
        await controller.requestImagePreview(
          'one',
          bounds: const HistoryImagePreviewBounds(width: 900, height: 640),
        );
        await controller.requestImagePreview(
          'one',
          bounds: const HistoryImagePreviewBounds(width: 400, height: 320),
        );
        await controller.requestImagePreview(
          'other',
          bounds: const HistoryImagePreviewBounds(width: 900, height: 320),
        );
        expect(repository.imagePreviewEdges, hasLength(4));
        controller.suspend();
        expect(
          await controller.requestImagePreview(
            'one',
            bounds: const HistoryImagePreviewBounds(width: 900, height: 320),
          ),
          isNull,
        );
      },
    );

    test(
      'opening a large detail does not retain its body in history rows',
      () async {
        final body = 'x' * (2 * 1024 * 1024);
        final repository = _HistoryRepository()
          ..pages.add(Future.value(HistoryClipPage(items: [_clip('large')])))
          ..detailClip = HistoryClip(
            id: 'large',
            contentType: 'text/plain',
            preview: body,
            body: body,
            createdAt: DateTime.utc(2026),
            pinned: false,
          );
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        await controller.initialize();
        await controller.select('large');
        expect(controller.selectedClip?.body, body);
        expect(controller.items.single.body, isNull);
        expect(controller.items.single.preview.length, 1024);
        expect((await controller.spoilerContent('large')).body, isNull);
        expect((await controller.spoilerContent('large')).preview.length, 1024);
      },
    );

    test(
      'media completion updates its future without rebuilding all History',
      () async {
        final repository = _HistoryRepository();
        final image = Completer<HistoryImagePreview?>();
        final icon = Completer<HistorySourceAppIcon?>();
        repository.imagePreviewFutures.add(image.future);
        repository.sourceIconFuture = icon.future;
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        var rebuilds = 0;
        controller.addListener(() => rebuilds++);
        final loadedImage = controller.requestImagePreview('image');
        final loadedIcon = controller.requestSourceIcon('icon');
        final imageValue = HistoryImagePreview(
          Uint8List(4),
          width: 1,
          height: 1,
        );
        final iconValue = HistorySourceAppIcon(Uint8List(4));
        image.complete(imageValue);
        icon.complete(iconValue);
        expect(await loadedImage, same(imageValue));
        expect(await loadedIcon, same(iconValue));
        expect(rebuilds, 0);
      },
    );

    test(
      'a small byte view cannot hide an oversized retained backing buffer',
      () async {
        final repository = _HistoryRepository();
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        final allocation = Uint8List(17 * 1024 * 1024);
        repository.imagePreviewFutures.add(
          Future.value(
            HistoryImagePreview(
              Uint8List.sublistView(allocation, 0, 1),
              width: 1,
              height: 1,
            ),
          ),
        );
        await controller.requestImagePreview('view');
        await controller.requestImagePreview('view');
        expect(repository.imagePreviewEdges, hasLength(2));
      },
    );

    test(
      'preview cache evicts by bytes before its entry count limit',
      () async {
        final repository = _HistoryRepository();
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        for (final id in ['first', 'second', 'third']) {
          repository.imagePreviewFutures.add(
            Future.value(
              HistoryImagePreview(
                Uint8List(6 * 1024 * 1024),
                width: 2048,
                height: 2048,
              ),
            ),
          );
          await controller.requestImagePreview(id);
        }
        expect(repository.imagePreviewEdges, hasLength(3));
        await controller.requestImagePreview('second');
        expect(repository.imagePreviewEdges, hasLength(3));
        await controller.requestImagePreview('first');
        expect(repository.imagePreviewEdges, hasLength(4));
      },
    );

    test(
      'oversized preview is returned without flushing reusable thumbnails',
      () async {
        final repository = _HistoryRepository();
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        final small = HistoryImagePreview(Uint8List(32), width: 2, height: 2);
        final large = HistoryImagePreview(
          Uint8List(17 * 1024 * 1024),
          width: 2048,
          height: 2048,
        );
        repository.imagePreviewFutures.addAll([
          Future.value(small),
          Future.value(large),
        ]);
        expect(await controller.requestImagePreview('small'), same(small));
        expect(await controller.requestImagePreview('large'), same(large));
        expect(await controller.requestImagePreview('small'), same(small));
        expect(repository.imagePreviewEdges, hasLength(2));
        await controller.requestImagePreview('large');
        expect(repository.imagePreviewEdges, hasLength(3));
      },
    );

    test(
      'suspension releases payloads and restores the same search and selection',
      () async {
        final repository = _HistoryRepository()
          ..pages.addAll([
            Future.value(
              HistoryClipPage(items: [_clip('before')], nextCursor: 'next'),
            ),
            Future.value(HistoryClipPage(items: [_clip('after')])),
          ]);
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        await controller.updateQuery(
          const HistoryQuery(search: 'saved', sort: HistorySort.oldest),
        );
        await controller.initialize();
        await controller.select('after');
        controller.toggleSection('today');
        controller.suspend();
        expect(controller.items, isEmpty);
        expect(controller.selectedClip, isNull);
        expect(controller.selectedId, 'after');
        expect(controller.query.search, 'saved');
        expect(controller.query.sort, HistorySort.oldest);
        expect(controller.isSectionCollapsed('today'), isTrue);
        final reads = repository.requests.length;
        repository.events.add(HistoryRuntimeEvent.itemsChanged);
        await Future<void>.delayed(Duration.zero);
        expect(repository.requests, hasLength(reads));
        repository.pages.add(
          Future.value(HistoryClipPage(items: [_clip('fresh')])),
        );
        await controller.initialize();
        expect(controller.items.single.id, 'fresh');
        expect(controller.selectedClip?.id, 'after');
        expect(repository.requests.last.query.search, 'saved');
      },
    );

    test('stale media cannot repopulate caches or erase resumed in-flight requests', () async {
      final repository = _HistoryRepository();
      final oldIcon = Completer<HistorySourceAppIcon?>();
      final newIcon = Completer<HistorySourceAppIcon?>();
      final oldImage = Completer<HistoryImagePreview?>();
      repository.sourceIconFuture = oldIcon.future;
      repository.imagePreviewFutures.add(oldImage.future);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      final staleIconRequest = controller.requestSourceIcon('icon');
      final staleImageRequest = controller.requestImagePreview('image');
      controller.suspend();
      expect(await controller.requestSourceIcon('icon'), isNull);
      expect(await controller.requestImagePreview('image'), isNull);
      expect(repository.sourceIconCalls, 1);
      repository.pages.add(Future.value(const HistoryClipPage(items: [])));
      await controller.initialize();
      repository.sourceIconFuture = newIcon.future;
      final resumedIconRequest = controller.requestSourceIcon('icon');
      oldIcon.complete(HistorySourceAppIcon(Uint8List(1024)));
      oldImage.complete(
        HistoryImagePreview(Uint8List(1024), width: 16, height: 16),
      );
      expect(await staleIconRequest, isNull);
      expect(await staleImageRequest, isNull);
      expect(
        identical(controller.requestSourceIcon('icon'), resumedIconRequest),
        isTrue,
      );
      final icon = HistorySourceAppIcon(Uint8List(64));
      newIcon.complete(icon);
      expect(await resumedIconRequest, same(icon));
      expect(await controller.requestSourceIcon('icon'), same(icon));
      expect(repository.sourceIconCalls, 2);
    });

    test(
      'query and facet responses completing while suspended are discarded',
      () async {
        final repository = _HistoryRepository();
        final page = Completer<HistoryClipPage>();
        final facets = Completer<HistoryFacets>();
        repository.facetsFuture = facets.future;
        repository.pages.add(page.future);
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        final loading = controller.initialize();
        controller.suspend();
        page.complete(HistoryClipPage(items: [_clip('stale')]));
        facets.complete(
          const HistoryFacets(
            sourceApps: [
              HistorySourceAppFacet(
                id: 'stale',
                label: 'Stale',
                iconId: 'icon',
              ),
            ],
          ),
        );
        await loading;
        expect(controller.items, isEmpty);
        expect(controller.state, HistoryLoadState.initial);
        expect(controller.facets.sourceApps, isEmpty);
      },
    );

    test(
      'starts search with relevance and preserves an explicit search sort',
      () async {
        final repository = _HistoryRepository()
          ..pages.addAll(
            List.generate(
              5,
              (_) => Future.value(const HistoryClipPage(items: [])),
            ),
          );
        final controller = HistoryController(
          repository,
          searchDebounce: Duration.zero,
        );
        addTearDown(controller.dispose);

        controller.updateSearch('meeting');
        await Future<void>.delayed(Duration.zero);
        expect(controller.query.sort, HistorySort.relevance);

        await controller.updateQuery(
          controller.query.copyWith(sort: HistorySort.oldest),
        );
        controller.updateSearch('meting');
        await Future<void>.delayed(Duration.zero);
        expect(controller.query.sort, HistorySort.oldest);

        await controller.updateQuery(
          controller.query.copyWith(sort: HistorySort.relevance),
        );
        controller.updateSearch('');
        await Future<void>.delayed(Duration.zero);
        expect(controller.query.sort, HistorySort.newest);
        expect(repository.requests, hasLength(5));
        expect(controller.state, HistoryLoadState.empty);
      },
    );

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
      final before = controller.items;
      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.items.single.id, 'after');
      expect(controller.items, isNot(same(before)));
      expect(before.single.id, 'before');
      controller.dispose();
    });

    test('refresh retains the snapshot when all rows are unchanged', () async {
      final repository = _HistoryRepository();
      repository.pages.addAll([
        Future.value(HistoryClipPage(items: [_clip('one'), _clip('two')])),
        Future.value(HistoryClipPage(items: [_clip('one'), _clip('two')])),
      ]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      final before = controller.items;

      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.items, same(before));
    });

    test('refresh retains only unchanged row objects', () async {
      final repository = _HistoryRepository();
      repository.pages.addAll([
        Future.value(HistoryClipPage(items: [_clip('same'), _clip('changed')])),
        Future.value(
          HistoryClipPage(
            items: [_clip('same'), _clip('changed').copyWith(pinned: true)],
          ),
        ),
      ]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      final before = controller.items;

      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.items[0], same(before[0]));
      expect(controller.items[1], isNot(same(before[1])));
      expect(controller.items[1].pinned, isTrue);
    });

    test('refresh reuses unchanged rows after a reorder', () async {
      final repository = _HistoryRepository();
      repository.pages.addAll([
        Future.value(HistoryClipPage(items: [_clip('one'), _clip('two')])),
        Future.value(HistoryClipPage(items: [_clip('two'), _clip('one')])),
      ]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      final before = controller.items;

      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.items[0], same(before[1]));
      expect(controller.items[1], same(before[0]));
      expect(controller.items, isNot(same(before)));
    });

    test('refresh replaces a row when file metadata changes', () async {
      HistoryClip file(bool available) => HistoryClip(
        id: 'file',
        contentType: 'file',
        preview: 'file',
        createdAt: DateTime.utc(2026),
        pinned: false,
        file: HistoryFileDetails(name: 'file', sourceAvailable: available),
      );
      final repository = _HistoryRepository();
      repository.pages.addAll([
        Future.value(HistoryClipPage(items: [file(false)])),
        Future.value(HistoryClipPage(items: [file(true)])),
      ]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      final before = controller.items.single;

      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.items.single, isNot(same(before)));
      expect(controller.items.single.file?.sourceAvailable, isTrue);
    });

    test('reconnects a completed watch and refreshes missed history', () async {
      final first = StreamController<HistoryRuntimeEvent>.broadcast();
      final second = StreamController<HistoryRuntimeEvent>.broadcast();
      final repository = _RestartingWatchRepository([first, second]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      addTearDown(second.close);

      await controller.initialize();
      expect(repository.watchCalls, 1);
      repository.current = _clip('after-reconnect');
      await first.close();
      await Future<void>.delayed(const Duration(milliseconds: 150));
      expect(repository.watchCalls, 2);
      expect(controller.items.single.id, 'after-reconnect');
      expect(controller.errorMessage, isNull);
    });

    test(
      'suspension cancels the watch before initialize opens a replacement',
      () async {
        final first = StreamController<HistoryRuntimeEvent>.broadcast();
        final second = StreamController<HistoryRuntimeEvent>.broadcast();
        final repository = _RestartingWatchRepository([first, second]);
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        addTearDown(first.close);
        addTearDown(second.close);

        await controller.initialize();
        expect(first.hasListener, isTrue);
        controller.suspend();
        await Future<void>.delayed(Duration.zero);
        expect(first.hasListener, isFalse);

        await controller.initialize();
        expect(repository.watchCalls, 2);
      },
    );

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

    test('keys bounded previews by both dimensions', () async {
      final repository = _HistoryRepository();
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      const firstBounds = HistoryImagePreviewBounds(width: 1400, height: 320);
      await controller.requestImagePreview('one', bounds: firstBounds);
      await controller.requestImagePreview('one', bounds: firstBounds);
      await controller.requestImagePreview(
        'one',
        bounds: const HistoryImagePreviewBounds(width: 1400, height: 640),
      );
      await controller.requestImagePreview('one', maxEdge: 1024);

      expect(
        repository.imagePreviewBounds.map(
          (bounds) => (bounds?.width, bounds?.height),
        ),
        [(1400, 320), (1400, 640), (null, null)],
      );
      expect(repository.imagePreviewEdges, [null, null, 1024]);
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
        final firstSourceIcon = controller.requestSourceIcon('app:one');
        final repeatedSourceIcon = controller.requestSourceIcon('app:one');

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
      'retains loaded source icons and bounds retired cache entries',
      () async {
        final repository = _HistoryRepository();
        final clips = [
          for (var index = 0; index < 50; index++) _clip('icon-$index'),
        ];
        repository.pages.addAll([
          Future.value(HistoryClipPage(items: clips)),
          Future.value(HistoryClipPage(items: [_clip('next')])),
          Future.value(HistoryClipPage(items: clips)),
        ]);
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        await controller.initialize();
        await Future.wait([
          for (final clip in clips)
            controller.requestSourceIcon(clip.sourceAppIconId),
        ]);
        await Future.wait([
          for (final clip in clips)
            controller.requestSourceIcon(clip.sourceAppIconId),
        ]);
        expect(repository.sourceIconCalls, 50);

        await controller.updateQuery(const HistoryQuery(search: 'next'));
        await controller.updateQuery(const HistoryQuery());
        await controller.requestSourceIcon('app:icon-0');
        expect(repository.sourceIconCalls, 51);
        await controller.requestSourceIcon('app:icon-49');
        expect(repository.sourceIconCalls, 51);
      },
    );

    test('shared icon requests survive a history query change', () async {
      final repository = _HistoryRepository();
      final pending = Completer<HistorySourceAppIcon?>();
      repository.sourceIconFuture = pending.future;
      repository.pages.addAll([
        Future.value(HistoryClipPage(items: [_clip('one')])),
        Future.value(HistoryClipPage(items: [_clip('two')])),
      ]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      final first = controller.requestSourceIcon('app:shared');
      await controller.updateQuery(const HistoryQuery(search: 'two'));
      final second = controller.requestSourceIcon('app:shared');
      expect(identical(first, second), isTrue);
      final icon = HistorySourceAppIcon(Uint8List.fromList([1, 2, 3]));
      pending.complete(icon);
      expect(await first, same(icon));
      expect(await second, same(icon));
      expect(await controller.requestSourceIcon('app:shared'), same(icon));
      expect(repository.sourceIconCalls, 1);
    });

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

    test('deletes a clip without changing another clip selection', () async {
      final repository = _HistoryRepository();
      repository.pages.add(
        Future.value(HistoryClipPage(items: [_clip('one'), _clip('two')])),
      );
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.select('one');

      expect(await controller.deleteClip('two'), isTrue);
      expect(controller.items.map((clip) => clip.id), ['one']);
      expect(controller.selectedId, 'one');
      expect(controller.selectedClip?.id, 'one');
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
        final pendingSave = Completer<void>();
        final pendingPresentation = Completer<void>();
        repository.savePending = pendingSave.future;
        downloader.presentPending = pendingPresentation.future;
        var broadNotifications = 0;
        final progress = <double?>[];
        final broadAtProgress = <int>[];
        controller.addListener(() => broadNotifications++);
        controller.downloadProgressChanges.addListener(() {
          progress.add(controller.downloadProgressFor(file.id));
          broadAtProgress.add(broadNotifications);
        });
        final result = controller.downloadSelected();
        await Future<void>.delayed(Duration.zero);
        expect(progress, [0]);
        expect(broadNotifications, 1);
        pendingSave.complete();
        await Future<void>.delayed(Duration.zero);
        expect(progress, [0, 1]);
        expect(broadAtProgress, [1, 1]);
        expect(broadNotifications, 1);
        pendingPresentation.complete();
        expect(await result, HistoryFileDownloadResult.saved);
        expect(broadNotifications, 2);
        expect(controller.downloadProgressFor(file.id), isNull);
        expect(repository.savedFiles, [('file', '/tmp/report.pdf')]);
        expect(downloader.presentedPaths, ['/tmp/report.pdf']);

        repository.savePending = null;
        repository.saveError = StateError('save failed');
        expect(
          await controller.downloadSelected(),
          HistoryFileDownloadResult.failed,
        );
        expect(controller.downloadFailedFor(file.id), isTrue);

        final pendingDestination = Completer<String?>();
        downloader.destinationPending = pendingDestination.future;
        final cancelled = controller.downloadSelected();
        controller.cancelDownload(file.id);
        pendingDestination.complete('/tmp/report.pdf');
        expect(await cancelled, HistoryFileDownloadResult.cancelled);
        expect(controller.isDownloadPending(file.id), isFalse);
        expect(controller.downloadFailedFor(file.id), isFalse);
        controller.dispose();
      },
    );
  });
}

HistoryClip _clip(String id) => HistoryClip(
  id: id,
  sourceAppIconId: 'app:$id',
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
  @override
  Future<void> importFile(HistoryImportFile file) async {}

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
  Future<HistoryFacets>? facetsFuture;
  final List<Future<HistoryImagePreview?>> imagePreviewFutures = [];
  final List<int?> imagePreviewEdges = [];
  final List<HistoryImagePreviewBounds?> imagePreviewBounds = [];
  Future<HistorySourceAppIcon?>? sourceIconFuture;
  int sourceIconCalls = 0;
  HistoryClip? detailClip;
  final List<(String, String)> savedFiles = [];
  Future<void>? savePending;
  Object? saveError;

  @override
  Stream<HistoryRuntimeEvent> watch() => events.stream;

  @override
  Future<HistoryFacets> facets() =>
      facetsFuture ?? Future.value(availableFacets);

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
  Future<HistoryImagePreview?> imagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) {
    imagePreviewEdges.add(maxEdge);
    imagePreviewBounds.add(bounds);
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
  Future<void> copyText(String id, String text) async {}

  @override
  Future<void> copyPlainText(String id) async {}

  @override
  Future<void> saveFile(String id, String destinationPath) async {
    savedFiles.add((id, destinationPath));
    await savePending;
    if (saveError != null) throw saveError!;
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
  Future<HistoryClip> setSecret(String id, bool secret) async =>
      throw UnimplementedError('setSecret');

  @override
  Future<void> setPinned(String id, bool pinned) async {
    setPinnedCalls++;
    await setPinnedPending;
    if (setPinnedError != null) throw setPinnedError!;
  }
}

class _RestartingWatchRepository extends Fake implements HistoryRepository {
  _RestartingWatchRepository(this._watches);

  final List<StreamController<HistoryRuntimeEvent>> _watches;
  int watchCalls = 0;
  HistoryClip current = _clip('initial');

  @override
  Stream<HistoryRuntimeEvent> watch() => _watches[watchCalls++].stream;

  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async => HistoryClipPage(items: [current]);

  @override
  Future<HistoryClip> get(String id) async => current;
}

class _HistoryFileDownloader implements HistoryFileDownloader {
  _HistoryFileDownloader(this.destination);

  final String? destination;
  final List<String> presentedPaths = [];
  Future<void>? presentPending;
  Future<String?>? destinationPending;

  @override
  Future<String?> chooseDestination(HistoryFileDetails file) async =>
      destinationPending == null ? destination : await destinationPending;

  @override
  Future<void> presentSavedFile(String path, HistoryFileDetails file) async {
    presentedPaths.add(path);
    await presentPending;
  }
}
