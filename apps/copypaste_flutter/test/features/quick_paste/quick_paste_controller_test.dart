import 'dart:async';

import 'package:flutter/services.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/source_app_label.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/quick_paste/quick_paste_app.dart';
import 'package:copypaste_flutter/features/quick_paste/quick_paste_controller.dart';
import 'package:copypaste_flutter/features/settings/repository/quick_paste_preferences_store.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as image;
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('renders the keyboard-first popup and footer actions', (
    tester,
  ) async {
    final controller = QuickPasteController(
      repository: _Repository(),
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );

    await tester.binding.setSurfaceSize(const Size(448, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle(const Duration(milliseconds: 150));

    expect(find.text('Type to search…'), findsOneWidget);
    expect(find.text('Pinned clip'), findsOneWidget);
    expect(find.text('Recent clip'), findsOneWidget);
    final sourceApps = find.byType(SourceAppLabel);
    expect(sourceApps, findsNWidgets(2));
    expect(
      tester.widgetList<SourceAppLabel>(sourceApps).map((label) => label.name),
      everyElement('Editor'),
    );
    expect(find.text('Clear'), findsOneWidget);
    expect(find.text('Preferences…'), findsOneWidget);
    expect(find.text('About'), findsOneWidget);
    expect(find.text('Quit'), findsOneWidget);
    expect(find.byType(Command), findsNothing);
    expect(find.byType(Alert), findsNothing);
    expect(find.text('CopyPaste'), findsNothing);
    expect(
      tester.getSize(find.byKey(const ValueKey('quick-paste-logo'))),
      const Size(16, 16),
    );
    final logo = tester.getRect(find.byKey(const ValueKey('quick-paste-logo')));
    final search = tester.getRect(
      find.byKey(const ValueKey('quick-paste-search')),
    );
    final inspectorToggle = tester.getRect(
      find.byKey(const ValueKey('quick-paste-inspector-toggle')),
    );
    expect(logo.center.dy, closeTo(search.center.dy, 0.1));
    expect(logo.center.dy, closeTo(inspectorToggle.center.dy, 0.1));
    expect(search.left - logo.right, 4);
    expect(
      tester.getTopLeft(find.text('Pinned clip')).dy,
      greaterThan(tester.getTopLeft(find.text('Recent clip')).dy),
    );
    expect(
      tester.getTopLeft(find.text('Preferences…')).dy,
      greaterThan(tester.getTopLeft(find.text('Clear')).dy),
    );
    expect(
      tester
          .widgetList<SourceAppLabel>(sourceApps)
          .map((label) => label.showName),
      everyElement(false),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('image rows show large content previews without image labels', (
    tester,
  ) async {
    final repository = _Repository();
    repository.clips.insert(
      0,
      HistoryClip(
        id: 'image',
        contentType: 'image/png',
        preview: '[image]',
        createdAt: DateTime.utc(2026),
        pinned: false,
        sourceApp: 'Editor',
      ),
    );
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    await tester.binding.setSurfaceSize(const Size(448, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    final preview = find.descendant(
      of: find.byKey(const ValueKey('quick-paste-image')),
      matching: find.byType(Image),
    );
    expect(find.text('[image]'), findsNothing);
    expect(tester.getSize(preview).height, 120);
    expect(repository.imagePreviewEdges, contains(480));
    expect(tester.takeException(), isNull);
  });

  testWidgets('arrows select the visible menu order while search keeps focus', (
    tester,
  ) async {
    final repository = _Repository();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    await tester.binding.setSurfaceSize(const Size(448, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    await controller.opened(1);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(controller.focusedClip?.id, 'recent');
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(controller.focusedClip?.id, 'pinned');
    expect(
      tester
          .widget<TextField>(find.byKey(const ValueKey('quick-paste-search')))
          .focusNode
          ?.hasFocus,
      isTrue,
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(repository.copied, ['pinned']);
    expect(tester.takeException(), isNull);
  });

  test('number shortcuts activate the visible menu order', () async {
    final repository = _Repository();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    await controller.activateIndex(0);
    expect(repository.copied, ['recent']);
    await controller.opened(2);
    await controller.activateIndex(1);
    expect(repository.copied, ['recent']);
  });

  testWidgets('inspector shares History content and follows the focused clip', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    await tester.binding.setSurfaceSize(const Size(816, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    await controller.opened(1);
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('quick-paste-inspector-toggle')),
    );
    await tester.pumpAndSettle();
    expect(host.inspectorChanges, [true]);
    expect(controller.history.selectedClip?.id, 'recent');
    expect(
      find.byKey(const ValueKey('history-detail-inspector')),
      findsOneWidget,
    );
    controller.focus(repository.clips.first);
    await tester.pumpAndSettle();
    expect(controller.history.selectedClip?.id, 'pinned');
    await tester.tap(
      find.byKey(const ValueKey('history-detail-inspector-close')),
    );
    await tester.pumpAndSettle();
    expect(host.inspectorChanges, [true, false]);
    expect(
      find.byKey(const ValueKey('history-detail-inspector')),
      findsNothing,
    );
    expect(tester.takeException(), isNull);
  });

  test(
    'pinned letter shortcuts remain stable across filtering and restart',
    () async {
      final repository = _Repository();
      final store = MemoryQuickPastePreferencesStore();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: store,
        host: _ContextHost()..permissionGranted = true,
      );
      await controller.initialize();
      final pin = repository.clips.first;
      final key = controller.shortcutFor(pin);
      expect(key, isNotNull);
      expect(key, isNot(LogicalKeyboardKey.digit2));
      expect(controller.shortcuts[LogicalKeyboardKey.digit1]?.id, 'recent');
      expect(controller.shortcuts[key]?.id, 'pinned');
      await controller.opened(1);
      await controller.activate(controller.shortcuts[key]!);
      expect(repository.copied, ['pinned']);
      repository.clips.remove(pin);
      await controller.search('recent');
      repository.clips.add(pin);
      await controller.opened(2);
      expect(controller.shortcutFor(pin), key);
      controller.dispose();
      final restarted = QuickPasteController(
        repository: repository,
        preferencesStore: store,
        host: _ContextHost()..permissionGranted = true,
      );
      addTearDown(restarted.dispose);
      await restarted.initialize();
      expect(restarted.shortcutFor(pin), key);
    },
  );

  test(
    'pinned shortcuts are unique and exclude system and menu actions',
    () async {
      final repository = _Repository();
      repository.clips.add(
        HistoryClip(
          id: 'second-pin',
          contentType: 'text/plain',
          preview: 'Second pin',
          createdAt: DateTime.utc(2026),
          pinned: true,
        ),
      );
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost()..permissionGranted = true,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      final pinned = repository.clips.where((clip) => clip.pinned).toList();
      final keys = pinned.map(controller.shortcutFor).toSet();
      expect(keys.length, 2);
      expect(keys, isNot(contains(null)));
      for (final reserved in [
        LogicalKeyboardKey.keyA,
        LogicalKeyboardKey.keyV,
        LogicalKeyboardKey.keyQ,
        LogicalKeyboardKey.keyW,
        LogicalKeyboardKey.keyZ,
        LogicalKeyboardKey.keyP,
      ]) {
        expect(keys, isNot(contains(reserved)));
      }
    },
  );

  test(
    'Accessibility is requested only for paste and a denial survives reopen',
    () async {
      final store = MemoryQuickPastePreferencesStore();
      final repository = _Repository();
      final host = _ContextHost();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: store,
        host: host,
      );
      await controller.initialize();
      await controller.opened(1);
      await controller.opened(2);
      expect(host.permissionRequests, 0);
      await controller.activateIndex(0);
      expect(host.permissionRequests, 1);
      await controller.opened(3);
      await controller.activateIndex(0);
      expect(host.permissionRequests, 1);
      controller.dispose();

      final restarted = QuickPasteController(
        repository: repository,
        preferencesStore: store,
        host: host,
      );
      addTearDown(restarted.dispose);
      await restarted.initialize();
      await restarted.opened(4);
      await restarted.activateIndex(0);
      expect(host.permissionRequests, 1);
      expect(repository.copied, ['recent', 'recent', 'recent']);
      expect(host.pasteCalls, 0);
    },
  );

  test('copy-only selection never requests Accessibility', () async {
    final host = _ContextHost();
    final controller = QuickPasteController(
      repository: _Repository(),
      preferencesStore: MemoryQuickPastePreferencesStore(
        QuickPastePreferences(
          autoPaste: false,
          shortcut: DesktopShortcut.defaultForPlatform(),
        ),
      ),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    await controller.activateIndex(0);
    expect(host.permissionRequests, 0);
    expect(host.closeCalls, 1);
  });

  test('auto-pastes the copied clip when permission is available', () async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);

    final clip = controller.history.items.last;
    controller.focus(clip);
    await controller.activateFocused();

    expect(repository.copied, ['recent']);
    expect(host.pasteCalls, 1);
    expect(host.closeCalls, 0);
  });

  test('falls back to copy and close when Accessibility is denied', () async {
    final repository = _Repository();
    final host = _ContextHost();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);

    await controller.activate(controller.history.items.last);

    expect(host.permissionRequests, 1);
    expect(repository.copied, ['recent']);
    expect(host.pasteCalls, 0);
    expect(host.closeCalls, 1);
  });

  test('Option selection inverts copy-only mode for one paste', () async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(
        QuickPastePreferences(
          autoPaste: false,
          shortcut: DesktopShortcut.defaultForPlatform(),
        ),
      ),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);

    await controller.activate(
      controller.history.items.last,
      invertAutoPaste: true,
    );

    expect(host.pasteCalls, 1);
    expect(host.closeCalls, 0);
  });

  test('clear keeps pinned clips visible', () async {
    final repository = _Repository();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);

    expect(await controller.clearUnpinned(), isTrue);
    expect(controller.history.items.map((clip) => clip.id), ['pinned']);
    expect(repository.deleteAllCalls, 1);
  });
  test(
    'reopen during Copy keeps one owner and never queues another write',
    () async {
      final repository = _Repository();
      final host = _ContextHost()..permissionGranted = true;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.opened(1);
      repository.copyGate = Completer<void>();
      final oldAction = controller.activate(controller.history.items.last);
      await repository.copyStarted.future;
      await controller.opened(2);
      await controller.activate(controller.history.items.first);
      expect(controller.activating, isTrue);
      expect(repository.copied, ['recent']);
      repository.copyGate!.complete();
      await oldAction;
      expect(controller.activating, isFalse);
      expect(host.pasteIds, isEmpty);
      expect(host.closeIds, isEmpty);
      expect(repository.copied, ['recent']);

      repository.copyGate = Completer<void>();
      repository.copyStarted = Completer<void>();
      final newAction = controller.activate(controller.history.items.first);
      await repository.copyStarted.future;
      expect(controller.activating, isTrue);
      repository.copyGate!.complete();
      await newAction;
      expect(controller.activating, isFalse);
      expect(repository.copied, ['recent', 'pinned']);
      expect(host.pasteIds, [2]);
    },
  );

  test('reopen while selection is pending starts no stale Copy', () async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    repository.selectGate = Completer<HistoryClip>();
    final clip = controller.history.items.last;
    final action = controller.activate(clip);
    await repository.selectStarted.future;
    await controller.opened(2);
    expect(controller.activating, isTrue);
    await controller.activate(controller.history.items.first);
    repository.selectGate!.complete(clip);
    await action;
    expect(repository.copied, isEmpty);
    expect(host.pasteIds, isEmpty);
    expect(host.closeIds, isEmpty);
  });

  test(
    'native reopen before Dart opened rejects old paste and conditional close',
    () async {
      final repository = _Repository();
      final host = _ContextHost()..permissionGranted = true;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.opened(1);
      repository.copyGate = Completer<void>();
      final action = controller.activate(controller.history.items.last);
      await repository.copyStarted.future;
      host.nativeId = 2;
      repository.copyGate!.complete();
      await action;
      expect(repository.copied, ['recent']);
      expect(host.pasteIds, [1]);
      expect(host.closeIds, [1]);
      expect(host.actualCloseIds, isEmpty);
      expect(host.nativeId, 2);
    },
  );

  for (final failure in [
    false,
    PlatformException(code: 'gone'),
    MissingPluginException(),
  ]) {
    test(
      'optional paste failure $failure preserves one Copy and closes its ID',
      () async {
        final repository = _Repository();
        final host = _ContextHost()..permissionGranted = true;
        if (failure is bool) {
          host.pasteResult = failure;
        } else {
          host.pasteFailure = failure;
        }
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
        );
        addTearDown(controller.dispose);
        await controller.initialize();
        await controller.opened(1);
        await controller.activate(controller.history.items.last);
        expect(repository.copied, ['recent']);
        expect(host.pasteIds, [1]);
        expect(host.closeIds, [1]);
        expect(controller.activating, isFalse);
      },
    );
  }

  test(
    'old pending paste false cannot close a newer Dart presentation',
    () async {
      final repository = _Repository();
      final host = _ContextHost()..permissionGranted = true;
      host.pasteGate = Completer<bool>();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.opened(1);
      final action = controller.activate(controller.history.items.last);
      await host.pasteStarted.future;
      await controller.opened(2);
      await controller.activate(controller.history.items.first);
      expect(controller.activating, isTrue);
      host.pasteGate!.complete(false);
      await action;
      expect(repository.copied, ['recent']);
      expect(host.closeIds, isEmpty);
    },
  );

  for (final copyFails in [false, true]) {
    test(
      'disposal during Copy suppresses late host calls and notifications ($copyFails)',
      () async {
        final repository = _Repository();
        final host = _ContextHost()..permissionGranted = true;
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
        );
        await controller.initialize();
        await controller.opened(1);
        repository.copyGate = Completer<void>();
        final action = controller.activate(controller.history.items.last);
        await repository.copyStarted.future;
        var notifications = 0;
        controller.addListener(() {
          notifications++;
        });
        controller.dispose();
        if (copyFails) {
          repository.copyGate!.completeError(StateError('copy failed'));
        } else {
          repository.copyGate!.complete();
        }
        await action;
        expect(notifications, 0);
        expect(host.pasteIds, isEmpty);
        expect(host.closeIds, isEmpty);
        expect(host.openedHandler, isNull);
        expect(controller.activating, isFalse);
      },
    );
  }

  test(
    'disposal while selection is pending starts no Copy or host action',
    () async {
      final repository = _Repository();
      final host = _ContextHost()..permissionGranted = true;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      await controller.initialize();
      await controller.opened(1);
      repository.selectGate = Completer<HistoryClip>();
      final clip = controller.history.items.last;
      final action = controller.activate(clip);
      await repository.selectStarted.future;
      controller.dispose();
      repository.selectGate!.complete(clip);
      await action;
      expect(repository.copied, isEmpty);
      expect(host.pasteIds, isEmpty);
      expect(host.closeIds, isEmpty);
    },
  );

  test('copy-only mode closes after one Copy without native paste', () async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(
        QuickPastePreferences(
          autoPaste: false,
          shortcut: DesktopShortcut.defaultForPlatform(),
        ),
      ),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    await controller.activate(controller.history.items.last);
    expect(repository.copied, ['recent']);
    expect(host.pasteIds, isEmpty);
    expect(host.closeIds, [1]);
    await controller.opened(2);
    await controller.activate(controller.history.items.first, forcePaste: true);
    expect(host.pasteIds, [2]);
  });
}

class _Repository implements HistoryRepository {
  final clips = <HistoryClip>[
    HistoryClip(
      id: 'pinned',
      contentType: 'text/plain',
      preview: 'Pinned clip',
      createdAt: DateTime.utc(2026),
      pinned: true,
      sourceApp: 'Editor',
    ),
    HistoryClip(
      id: 'recent',
      contentType: 'text/plain',
      preview: 'Recent clip',
      createdAt: DateTime.utc(2026),
      pinned: false,
      sourceApp: 'Editor',
    ),
  ];
  final copied = <String>[];
  final imagePreviewEdges = <int?>[];
  int deleteAllCalls = 0;
  Completer<void>? copyGate;
  Completer<void> copyStarted = Completer<void>();
  Completer<HistoryClip>? selectGate;
  Completer<void> selectStarted = Completer<void>();

  @override
  Future<void> copy(String id) async {
    copied.add(id);
    if (!copyStarted.isCompleted) copyStarted.complete();
    await copyGate?.future;
  }

  @override
  Future<void> copyPlainText(String id) => copy(id);

  @override
  Future<void> saveFile(String id, String destinationPath) async {}

  @override
  Future<void> delete(String id) async {
    clips.removeWhere((clip) => clip.id == id);
  }

  @override
  Future<void> deleteAll() async {
    deleteAllCalls += 1;
    clips.removeWhere((clip) => !clip.pinned);
  }

  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();

  @override
  Future<HistoryClip> get(String id) async {
    if (!selectStarted.isCompleted) selectStarted.complete();
    return await selectGate?.future ??
        clips.singleWhere((clip) => clip.id == id);
  }

  @override
  Future<HistoryImagePreview?> imagePreview(String id, {int? maxEdge}) async {
    imagePreviewEdges.add(maxEdge);
    if (id != 'image') return null;
    final preview = image.Image(width: 240, height: 180);
    return HistoryImagePreview(
      Uint8List.fromList(image.encodePng(preview)),
      width: 240,
      height: 180,
    );
  }

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async => HistoryClipPage(items: List.of(clips));

  @override
  Future<void> reorderPinned(List<String> ids) async {}

  @override
  Future<void> setPinned(String id, bool pinned) async {}

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async => null;

  @override
  Stream<HistoryRuntimeEvent> watch() => const Stream.empty();
}

class _ContextHost implements QuickPasteContextHost {
  final inspectorChanges = <bool>[];

  @override
  Future<void> setInspectorVisible({
    required int presentationId,
    required bool visible,
  }) async {
    inspectorChanges.add(visible);
  }

  bool permissionGranted = false;
  int permissionRequests = 0;
  int pasteCalls = 0;
  int closeCalls = 0;
  Future<void> Function(int presentationId)? openedHandler;
  final pasteIds = <int>[];
  final closeIds = <int>[];
  final actualCloseIds = <int>[];
  int? nativeId;
  bool pasteResult = true;
  Object? pasteFailure;
  Completer<bool>? pasteGate;
  Completer<void> pasteStarted = Completer<void>();

  @override
  Future<bool> accessibilityGranted() async => permissionGranted;

  @override
  Future<void> close({required int presentationId}) async {
    closeCalls += 1;
    closeIds.add(presentationId);
    if (nativeId == null || nativeId == presentationId) {
      actualCloseIds.add(presentationId);
    }
  }

  @override
  Future<void> dispose() async {}

  @override
  Future<void> openMainWindow() async {}

  @override
  Future<void> openSettings() async {}

  @override
  Future<bool> paste({required int presentationId}) async {
    pasteCalls += 1;
    pasteIds.add(presentationId);
    if (!pasteStarted.isCompleted) pasteStarted.complete();
    if (pasteFailure case final Object error) throw error;
    if (nativeId != null && nativeId != presentationId) return false;
    return await pasteGate?.future ?? pasteResult;
  }

  @override
  Future<void> quit() async {}

  @override
  Future<bool> requestAccessibility() async {
    permissionRequests += 1;
    return permissionGranted;
  }

  @override
  void setOpenedHandler(Future<void> Function(int presentationId)? handler) {
    openedHandler = handler;
  }
}
