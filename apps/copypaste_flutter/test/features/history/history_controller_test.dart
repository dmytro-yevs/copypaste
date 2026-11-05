import 'dart:async';
import 'dart:typed_data';

import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_window_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_transfer_controller.dart';
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

      final first = controller.window.items;
      expect(controller.window.items, same(first));
      expect(() => first.clear(), throwsUnsupportedError);
      await controller.window.loadMore();
      final loaded = controller.window.items;
      expect(loaded, isNot(same(first)));
      expect(first.map((clip) => clip.id), ['one']);
      expect(loaded.map((clip) => clip.id), ['one', 'two']);
      expect(controller.window.items, same(loaded));

      expect(await controller.mutations.togglePin(loaded.first), isTrue);
      final pinned = controller.window.items;
      expect(pinned, isNot(same(loaded)));
      expect(loaded.first.pinned, isFalse);
      expect(pinned.first.pinned, isTrue);
      expect(await controller.mutations.deleteClip('two'), isTrue);
      final deleted = controller.window.items;
      expect(deleted, isNot(same(pinned)));
      expect(pinned.map((clip) => clip.id), ['one', 'two']);
      expect(deleted.map((clip) => clip.id), ['one']);

      controller.suspend();
      final suspended = controller.window.items;
      expect(suspended, isEmpty);
      expect(suspended, isNot(same(deleted)));
      expect(deleted.single.id, 'one');
      expect(controller.window.items, same(suspended));
      await controller.initialize();
      expect(controller.window.items.single.id, 'fresh');
      expect(controller.window.items, isNot(same(suspended)));
      expect(suspended, isEmpty);
      final resumed = controller.window.items;
      await controller.window.reload();
      expect(controller.window.items.single.id, 'reloaded');
      expect(resumed.single.id, 'fresh');
      expect(controller.window.items, isNot(same(resumed)));
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
          await controller.media.requestImagePreview(
            'one',
            bounds: const HistoryImagePreviewBounds(width: 1400, height: 320),
          ),
          same(preview),
        );
        expect(
          await controller.media.requestImagePreview(
            'one',
            bounds: const HistoryImagePreviewBounds(width: 900, height: 320),
          ),
          same(preview),
        );
        expect(repository.imagePreviewEdges, hasLength(1));
        await controller.media.requestImagePreview(
          'one',
          bounds: const HistoryImagePreviewBounds(width: 900, height: 640),
        );
        await controller.media.requestImagePreview(
          'one',
          bounds: const HistoryImagePreviewBounds(width: 400, height: 320),
        );
        await controller.media.requestImagePreview(
          'other',
          bounds: const HistoryImagePreviewBounds(width: 900, height: 320),
        );
        expect(repository.imagePreviewEdges, hasLength(4));
        controller.suspend();
        expect(
          await controller.media.requestImagePreview(
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
        await controller.selection.select('large');
        expect(controller.selection.selectedClip?.body, body);
        expect(controller.window.items.single.body, isNull);
        expect(controller.window.items.single.preview.length, 1024);
        expect((await controller.media.spoilerContent('large')).body, isNull);
        expect(
          (await controller.media.spoilerContent('large')).preview.length,
          1024,
        );
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
        final loadedImage = controller.media.requestImagePreview('image');
        final loadedIcon = controller.media.requestSourceIcon('icon');
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
        await controller.media.requestImagePreview('view');
        await controller.media.requestImagePreview('view');
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
          await controller.media.requestImagePreview(id);
        }
        expect(repository.imagePreviewEdges, hasLength(3));
        await controller.media.requestImagePreview('second');
        expect(repository.imagePreviewEdges, hasLength(3));
        await controller.media.requestImagePreview('first');
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
        expect(
          await controller.media.requestImagePreview('small'),
          same(small),
        );
        expect(
          await controller.media.requestImagePreview('large'),
          same(large),
        );
        expect(
          await controller.media.requestImagePreview('small'),
          same(small),
        );
        expect(repository.imagePreviewEdges, hasLength(2));
        await controller.media.requestImagePreview('large');
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
        await controller.window.updateQuery(
          const HistoryQuery(search: 'saved', sort: HistorySort.oldest),
        );
        await controller.initialize();
        await controller.selection.select('after');
        controller.window.toggleSection('today');
        controller.suspend();
        expect(controller.window.items, isEmpty);
        expect(controller.selection.selectedClip, isNull);
        expect(controller.selection.selectedId, 'after');
        expect(controller.window.query.search, 'saved');
        expect(controller.window.query.sort, HistorySort.oldest);
        expect(controller.window.isSectionCollapsed('today'), isTrue);
        final reads = repository.requests.length;
        repository.events.add(HistoryRuntimeEvent.itemsChanged);
        await Future<void>.delayed(Duration.zero);
        expect(repository.requests, hasLength(reads));
        repository.pages.add(
          Future.value(HistoryClipPage(items: [_clip('fresh')])),
        );
        await controller.initialize();
        expect(controller.window.items.single.id, 'fresh');
        expect(controller.selection.selectedClip?.id, 'after');
        expect(repository.requests.last.query.search, 'saved');
      },
    );

    test(
      'stale media cannot repopulate caches or erase resumed in-flight requests',
      () async {
        final repository = _HistoryRepository();
        final oldIcon = Completer<HistorySourceAppIcon?>();
        final newIcon = Completer<HistorySourceAppIcon?>();
        final oldImage = Completer<HistoryImagePreview?>();
        repository.sourceIconFuture = oldIcon.future;
        repository.imagePreviewFutures.add(oldImage.future);
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        final staleIconRequest = controller.media.requestSourceIcon('icon');
        final staleImageRequest = controller.media.requestImagePreview('image');
        controller.suspend();
        expect(await controller.media.requestSourceIcon('icon'), isNull);
        expect(await controller.media.requestImagePreview('image'), isNull);
        expect(repository.sourceIconCalls, 1);
        repository.pages.add(Future.value(const HistoryClipPage(items: [])));
        await controller.initialize();
        repository.sourceIconFuture = newIcon.future;
        final resumedIconRequest = controller.media.requestSourceIcon('icon');
        oldIcon.complete(HistorySourceAppIcon(Uint8List(1024)));
        oldImage.complete(
          HistoryImagePreview(Uint8List(1024), width: 16, height: 16),
        );
        expect(await staleIconRequest, isNull);
        expect(await staleImageRequest, isNull);
        expect(
          identical(
            controller.media.requestSourceIcon('icon'),
            resumedIconRequest,
          ),
          isTrue,
        );
        final icon = HistorySourceAppIcon(Uint8List(64));
        newIcon.complete(icon);
        expect(await resumedIconRequest, same(icon));
        expect(await controller.media.requestSourceIcon('icon'), same(icon));
        expect(repository.sourceIconCalls, 2);
      },
    );

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
        expect(controller.window.items, isEmpty);
        expect(controller.window.state, HistoryLoadState.initial);
        expect(controller.window.facets.sourceApps, isEmpty);
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

        controller.window.updateSearch('meeting');
        await Future<void>.delayed(Duration.zero);
        expect(controller.window.query.sort, HistorySort.relevance);

        await controller.window.updateQuery(
          controller.window.query.copyWith(sort: HistorySort.oldest),
        );
        controller.window.updateSearch('meting');
        await Future<void>.delayed(Duration.zero);
        expect(controller.window.query.sort, HistorySort.oldest);

        await controller.window.updateQuery(
          controller.window.query.copyWith(sort: HistorySort.relevance),
        );
        controller.window.updateSearch('');
        await Future<void>.delayed(Duration.zero);
        expect(controller.window.query.sort, HistorySort.newest);
        expect(repository.requests, hasLength(5));
        expect(controller.window.state, HistoryLoadState.empty);
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
        'Credentials',
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
        HistoryClipKind.credentials,
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
      unawaited(
        controller.window.updateQuery(const HistoryQuery(search: 'second')),
      );
      await Future<void>.delayed(Duration.zero);

      second.complete(HistoryClipPage(items: [_clip('current')]));
      await Future<void>.delayed(Duration.zero);
      first.complete(HistoryClipPage(items: [_clip('stale')]));
      await Future<void>.delayed(Duration.zero);

      expect(controller.window.items.single.id, 'current');
      expect(controller.window.query.search, 'second');
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
        await controller.window.loadMore();

        expect(repository.requests, hasLength(2));
        expect(repository.requests.last.cursor, 'opaque-cursor');
        expect(repository.requests.last.query.search, isEmpty);
        expect(controller.window.items.map((item) => item.id), ['one', 'two']);
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

      unawaited(controller.window.loadMore());
      await Future<void>.delayed(Duration.zero);
      await controller.window.updateQuery(const HistoryQuery(search: 'new'));
      expect(controller.window.isLoadingMore, isFalse);

      await controller.window.loadMore();
      stalePage.complete(HistoryClipPage(items: [_clip('stale')]));
      await Future<void>.delayed(Duration.zero);

      expect(repository.requests.last.cursor, 'new-cursor');
      expect(controller.window.items.map((item) => item.id), ['new', 'newer']);
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
      final before = controller.window.items;
      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.window.items.single.id, 'after');
      expect(controller.window.items, isNot(same(before)));
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
      final before = controller.window.items;

      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.window.items, same(before));
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
      final before = controller.window.items;

      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.window.items[0], same(before[0]));
      expect(controller.window.items[1], isNot(same(before[1])));
      expect(controller.window.items[1].pinned, isTrue);
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
      final before = controller.window.items;

      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.window.items[0], same(before[1]));
      expect(controller.window.items[1], same(before[0]));
      expect(controller.window.items, isNot(same(before)));
    });

    test(
      'refresh keeps the loaded selection until its summary changes',
      () async {
        HistoryClip detail({required bool pinned}) => HistoryClip(
          id: 'one',
          sourceAppIconId: 'app:one',
          contentType: 'text/plain',
          preview: 'one',
          body: 'one',
          createdAt: DateTime.utc(2026),
          pinned: pinned,
        );
        final repository = _HistoryRepository()
          ..detailClip = detail(pinned: false)
          ..pages.addAll([
            Future.value(HistoryClipPage(items: [_clip('one')])),
            Future.value(HistoryClipPage(items: [_clip('one')])),
            Future.value(
              HistoryClipPage(items: [_clip('one').copyWith(pinned: true)]),
            ),
          ]);
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        await controller.initialize();
        await controller.selection.select('one');
        final loaded = controller.selection.selectedClip;
        expect(loaded?.body, 'one');

        repository.events.add(HistoryRuntimeEvent.itemsChanged);
        await Future<void>.delayed(Duration.zero);
        expect(controller.selection.selectedClip, same(loaded));
        expect(repository.getCalls, 1);

        repository.detailClip = detail(pinned: true);
        repository.events.add(HistoryRuntimeEvent.itemsChanged);
        await Future<void>.delayed(Duration.zero);
        expect(repository.getCalls, 2);
        expect(controller.selection.selectedClip?.pinned, isTrue);
        expect(controller.selection.selectedClip?.body, 'one');
      },
    );

    test('coalesces a burst of watch events into one follow-up', () async {
      final running = Completer<HistoryClipPage>();
      final repository = _HistoryRepository()
        ..pages.addAll([
          Future.value(HistoryClipPage(items: [_clip('one')])),
          running.future,
          Future.value(
            HistoryClipPage(
              items: [_clip('three'), _clip('two'), _clip('one')],
            ),
          ),
        ]);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();

      for (var event = 0; event < 5; event++) {
        repository.events.add(HistoryRuntimeEvent.itemsChanged);
      }
      await Future<void>.delayed(Duration.zero);
      expect(repository.requests, hasLength(2));
      running.complete(HistoryClipPage(items: [_clip('two'), _clip('one')]));
      await Future<void>.delayed(Duration.zero);

      expect(repository.requests, hasLength(3));
      expect(controller.window.items.map((clip) => clip.id), [
        'three',
        'two',
        'one',
      ]);
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
      final before = controller.window.items.single;

      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);

      expect(controller.window.items.single, isNot(same(before)));
      expect(controller.window.items.single.file?.sourceAvailable, isTrue);
    });

    test(
      'keeps the self-reconnecting watch and refreshes missed history',
      () async {
        final watch = StreamController<HistoryRuntimeEvent>.broadcast();
        final repository = _RestartingWatchRepository([watch]);
        final controller = HistoryController(repository);
        addTearDown(controller.dispose);
        addTearDown(watch.close);

        await controller.initialize();
        watch.addError(StateError('Runtime watch disconnected'));
        await Future<void>.delayed(Duration.zero);
        expect(controller.errors.message, isNotNull);
        expect(watch.hasListener, isTrue);
        repository.current = _clip('after-reconnect');
        watch.add(HistoryRuntimeEvent.itemsChanged);
        await Future<void>.delayed(Duration.zero);
        expect(repository.watchCalls, 1);
        expect(controller.window.items.single.id, 'after-reconnect');
        expect(controller.errors.message, isNull);
      },
    );

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

      expect(controller.window.state, HistoryLoadState.empty);
      expect(controller.window.skippedUndecryptable, 2);
      controller.dispose();
    });

    test('keys bounded previews by both dimensions', () async {
      final repository = _HistoryRepository();
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      const firstBounds = HistoryImagePreviewBounds(width: 1400, height: 320);
      await controller.media.requestImagePreview('one', bounds: firstBounds);
      await controller.media.requestImagePreview('one', bounds: firstBounds);
      await controller.media.requestImagePreview(
        'one',
        bounds: const HistoryImagePreviewBounds(width: 1400, height: 640),
      );
      await controller.media.requestImagePreview('one', maxEdge: 1024);

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

        final firstThumbnail = controller.media.requestImagePreview(
          'one',
          maxEdge: 96,
        );
        final repeatedThumbnail = controller.media.requestImagePreview(
          'one',
          maxEdge: 96,
        );
        final detailPreview = controller.media.requestImagePreview(
          'one',
          maxEdge: 1024,
        );
        final firstSourceIcon = controller.media.requestSourceIcon('app:one');
        final repeatedSourceIcon = controller.media.requestSourceIcon(
          'app:one',
        );

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
            controller.media.requestSourceIcon(clip.sourceAppIconId),
        ]);
        await Future.wait([
          for (final clip in clips)
            controller.media.requestSourceIcon(clip.sourceAppIconId),
        ]);
        expect(repository.sourceIconCalls, 50);

        await controller.window.updateQuery(const HistoryQuery(search: 'next'));
        await controller.window.updateQuery(const HistoryQuery());
        await controller.media.requestSourceIcon('app:icon-0');
        expect(repository.sourceIconCalls, 51);
        await controller.media.requestSourceIcon('app:icon-49');
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
      final first = controller.media.requestSourceIcon('app:shared');
      await controller.window.updateQuery(const HistoryQuery(search: 'two'));
      final second = controller.media.requestSourceIcon('app:shared');
      expect(identical(first, second), isTrue);
      final icon = HistorySourceAppIcon(Uint8List.fromList([1, 2, 3]));
      pending.complete(icon);
      expect(await first, same(icon));
      expect(await second, same(icon));
      expect(
        await controller.media.requestSourceIcon('app:shared'),
        same(icon),
      );
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

        expect(controller.window.facets.originDevices.single.label, 'Work Mac');
        expect(controller.window.facets.originDevices.single.id, 'device-1');
        expect(controller.window.facets.sourceApps.single.label, 'Editor');
        await controller.window.updateQuery(
          const HistoryQuery(
            origin: 'device-1',
            sourceApp: 'com.example.editor',
          ),
        );
        expect(repository.requests.last.query.origin, 'device-1');
        expect(repository.requests.last.query.sourceApp, 'com.example.editor');
        await controller.window.updateQuery(
          controller.window.query.copyWith(
            clearOrigin: true,
            clearSourceApp: true,
          ),
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

      await controller.mutations.togglePin(controller.window.items.single);

      expect(controller.window.items.single.pinned, isFalse);
      expect(controller.errors.message, 'The pin could not be updated.');
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

      final first = controller.mutations.togglePin(
        controller.window.items.single,
      );
      final second = await controller.mutations.togglePin(
        controller.window.items.single,
      );
      expect(repository.setPinnedCalls, 1);
      expect(second, isFalse);
      expect(controller.mutations.isPinPending('one'), isTrue);

      pending.complete();
      expect(await first, isTrue);
      expect(controller.mutations.isPinPending('one'), isFalse);
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
      await controller.selection.select('one');

      expect(await controller.mutations.deleteClip('two'), isTrue);
      expect(controller.window.items.map((clip) => clip.id), ['one']);
      expect(controller.selection.selectedId, 'one');
      expect(controller.selection.selectedClip?.id, 'one');
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
      await controller.selection.select('one');

      final firstDelete = controller.mutations.deleteSelected();
      final secondDelete = await controller.mutations.deleteSelected();
      expect(repository.deleteCalls, 1);
      expect(secondDelete, isFalse);
      pendingDelete.completeError(StateError('offline'));
      expect(await firstDelete, isFalse);
      expect(controller.window.items.single.id, 'one');

      final pendingAll = Completer<void>();
      repository.deleteAllPending = pendingAll.future;
      final firstAll = controller.mutations.deleteAll();
      final secondAll = await controller.mutations.deleteAll();
      expect(repository.deleteAllCalls, 1);
      expect(secondAll, isFalse);
      pendingAll.completeError(StateError('offline'));
      expect(await firstAll, isFalse);
      expect(controller.window.items.single.id, 'one');
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
        await controller.selection.select(file.id);

        expect(controller.transfers.canDownloadSelected, isTrue);
        final pendingSave = Completer<void>();
        final pendingPresentation = Completer<void>();
        repository.savePending = pendingSave.future;
        downloader.presentPending = pendingPresentation.future;
        var broadNotifications = 0;
        final progress = <double?>[];
        final broadAtProgress = <int>[];
        controller.addListener(() => broadNotifications++);
        controller.transfers.downloadProgressChanges.addListener(() {
          progress.add(controller.transfers.downloadProgressFor(file.id));
          broadAtProgress.add(broadNotifications);
        });
        final result = controller.transfers.downloadSelected();
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
        expect(controller.transfers.downloadProgressFor(file.id), isNull);
        expect(repository.savedFiles, [('file', '/tmp/report.pdf')]);
        expect(downloader.presentedPaths, ['/tmp/report.pdf']);

        repository.savePending = null;
        repository.saveError = StateError('save failed');
        expect(
          await controller.transfers.downloadSelected(),
          HistoryFileDownloadResult.failed,
        );
        expect(controller.transfers.downloadFailedFor(file.id), isTrue);

        final pendingDestination = Completer<String?>();
        downloader.destinationPending = pendingDestination.future;
        final cancelled = controller.transfers.downloadSelected();
        controller.transfers.cancelDownload(file.id);
        pendingDestination.complete('/tmp/report.pdf');
        expect(await cancelled, HistoryFileDownloadResult.cancelled);
        expect(controller.transfers.isDownloadPending(file.id), isFalse);
        expect(controller.transfers.downloadFailedFor(file.id), isFalse);
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
  int getCalls = 0;
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
  Future<HistoryClip> get(String id) async {
    getCalls++;
    return detailClip ?? _clip(id);
  }

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
