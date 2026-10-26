import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';

import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:flutter/services.dart';
import 'package:flutter/gestures.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_downloader.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_transfer.dart';
import 'package:copypaste_flutter/features/history/presentation/source_app_label.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/quick_paste/quick_paste_app.dart';
import 'package:copypaste_flutter/features/quick_paste/quick_paste_controller.dart';
import 'package:copypaste_flutter/features/quick_paste/middle_ellipsis.dart';
import 'package:copypaste_flutter/features/settings/repository/quick_paste_preferences_store.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as image;
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'package:copypaste_flutter/features/history/controller/history_ocr_controller.dart';
import 'package:copypaste_flutter/features/history/repository/history_image_input.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/modules_repository.dart';
import 'package:copypaste_flutter/platform/clipboard/clipboard_writer.dart';
import 'package:copypaste_flutter/features/transfers/file_operations_controller.dart';

import '../modules/modules_test_support.dart';

void main() {
  testWidgets(
    'retained Windows engine releases hidden views and watches and reloads fresh sessions',
    (tester) async {
      final host = _ContextHost();
      final preferences = MemoryQuickPastePreferencesStore();
      final repositories = <_RetainedRepository>[];
      final operations = <_RetainedOperations>[];
      final controllers = <QuickPasteController>[];
      var disposed = 0;
      QuickPasteController create() {
        final repository = _RetainedRepository();
        repository.clips.removeWhere((clip) => clip.pinned);
        repositories.add(repository);
        repository.clips
          ..clear()
          ..add(
            HistoryClip(
              id: 'session-${repositories.length}',
              pinned: false,
              contentType: 'text',
              preview: 'Fresh session ${repositories.length}',
              createdAt: DateTime.utc(2026),
            ),
          );
        final transfers = _RetainedOperations();
        operations.add(transfers);
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: preferences,
          host: host,
          ownsHostLifecycle: false,
          fileOperations: FileOperationsController(transfers),
          disposeRepository: () async {
            disposed++;
          },
        );
        controllers.add(controller);
        return controller;
      }

      await tester.pumpWidget(
        QuickPasteApp.retained(createController: create, host: host),
      );
      await tester.pumpAndSettle();
      expect(host.readyCalls, 1);
      expect(repositories, isEmpty);
      await host.openedHandler!(1, inspectorVisible: false);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('quick-paste-search')), findsOneWidget);
      expect(repositories.single.activeWatches, 1);
      expect(operations.single.activeWatches, 1);
      await host.hiddenHandler!(1);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('quick-paste-search')), findsNothing);
      expect(repositories.single.activeWatches, 0);
      expect(operations.single.activeWatches, 0);
      expect(disposed, 1);
      preferences.value = preferences.value.copyWith(autoPaste: false);
      await host.openedHandler!(2, inspectorVisible: true);
      await tester.pumpAndSettle();
      expect(controllers, hasLength(2));
      expect(controllers.last.autoPaste, isFalse);
      expect(controllers.last.inspectorOpen, isTrue);
      expect(repositories.last.queryCalls, greaterThan(0));
      expect(controllers.last.items.single.id, 'session-2');
      expect(repositories.last.activeWatches, 1);
      expect(host.readyCalls, 1);
      await host.hiddenHandler!(1); // Old native hide cannot remove session 2.
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('quick-paste-search')), findsOneWidget);
      expect(disposed, 1);
      await host.hiddenHandler!(2);
      await tester.pumpAndSettle();
      expect(disposed, 2);
      expect(repositories.last.activeWatches, 0);
      expect(operations.last.activeWatches, 0);
      await tester.pumpWidget(const SizedBox.shrink());
    },
    variant: TargetPlatformVariant({TargetPlatform.windows}),
  );

  testWidgets(
    'retained Windows engine waits for old Copy without stale paste before reopening',
    (tester) async {
      final host = _ContextHost()..permissionGranted = true;
      final repositories = <_RetainedRepository>[];
      final controllers = <QuickPasteController>[];
      QuickPasteController create() {
        final repository = _RetainedRepository();
        repositories.add(repository);
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
          ownsHostLifecycle: false,
        );
        controllers.add(controller);
        return controller;
      }

      await tester.pumpWidget(
        QuickPasteApp.retained(createController: create, host: host),
      );
      await tester.pumpAndSettle();
      await host.openedHandler!(1, inspectorVisible: false);
      await tester.pumpAndSettle();
      final old = repositories.single;
      old.copyGate = Completer<void>();
      final action = controllers.single.activate(
        controllers.single.items.first,
      );
      await old.copyStarted.future;
      final hidden = host.hiddenHandler!(1);
      final reopened = host.openedHandler!(2, inspectorVisible: false);
      await tester.pump();
      expect(controllers, hasLength(1));
      old.copyGate!.complete();
      await action;
      await hidden;
      await reopened;
      await tester.pumpAndSettle();
      expect(controllers, hasLength(2));
      expect(old.activeWatches, 0);
      expect(host.pasteIds, isEmpty);
      await controllers.last.activate(controllers.last.items.first);
      expect(host.pasteIds, [2]);
      await tester.pumpWidget(const SizedBox.shrink());
    },
    variant: TargetPlatformVariant({TargetPlatform.windows}),
  );

  testWidgets(
    'retained Windows engine ignores late bootstrap completion after hide and reopen',
    (tester) async {
      final host = _ContextHost();
      final gate = Completer<void>();
      final controllers = <QuickPasteController>[];
      final repositories = <_RetainedRepository>[];
      QuickPasteController create() {
        final repository = _RetainedRepository();
        if (repositories.isEmpty) repository.queryGate = gate;
        repositories.add(repository);
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
          ownsHostLifecycle: false,
        );
        controllers.add(controller);
        return controller;
      }

      await tester.pumpWidget(
        QuickPasteApp.retained(createController: create, host: host),
      );
      await tester.pumpAndSettle();
      final first = host.openedHandler!(1, inspectorVisible: false);
      await tester.pump();
      await host.hiddenHandler!(1);
      await host.openedHandler!(2, inspectorVisible: false);
      await tester.pumpAndSettle();
      expect(controllers, hasLength(2));
      expect(repositories.first.activeWatches, 0);
      expect(repositories.last.activeWatches, 1);
      gate.complete();
      await first;
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('quick-paste-search')), findsOneWidget);
      expect(host.pasteIds, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
    variant: TargetPlatformVariant({TargetPlatform.windows}),
  );

  testWidgets(
    'retained Windows engine never reveals media from a hidden session',
    (tester) async {
      final host = _FrameHost();
      final gate = Completer<void>();
      var created = 0;
      QuickPasteController create() {
        final repository = _RetainedRepository();
        if (created++ == 0) {
          repository.imageLoadGate = gate;
          repository.clips.insert(
            0,
            HistoryClip(
              id: 'image',
              pinned: false,
              contentType: 'image/png',
              preview: '[image]',
              createdAt: DateTime.utc(2026),
            ),
          );
        }
        return QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
          ownsHostLifecycle: false,
        );
      }

      await tester.pumpWidget(
        QuickPasteApp.retained(createController: create, host: host),
      );
      await tester.pumpAndSettle();
      await host.openedHandler!(1, inspectorVisible: false);
      await tester.pump();
      await tester.pump();
      await host.hiddenHandler!(1);
      await host.openedHandler!(2, inspectorVisible: false);
      await tester.pumpAndSettle();
      gate.complete();
      await tester.pumpAndSettle();
      expect(host.presentedIds, [2]);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
    variant: TargetPlatformVariant({TargetPlatform.windows}),
  );

  test('inspector refreshes completed metadata on each opening, not in the background', () async {
    var loads = 0;
    final repository = MemoryModulesRepository();
    final modules = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
    );
    final controller = QuickPasteController(
      repository: _Repository(),
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
      initializeInspector: () {
        loads++;
        return modules.initialize();
      },
    );
    addTearDown(controller.dispose);
    addTearDown(modules.dispose);
    await controller.initialize();
    await controller.opened(1, inspectorVisible: true);
    await Future<void>.delayed(Duration.zero);
    expect(loads, 1);
    expect(modules.modules, isEmpty);
    // Another engine installs/enables a module while the popup is closed.
    repository.modules = [testModule];
    await controller.opened(2, inspectorVisible: true);
    await Future<void>.delayed(Duration.zero);
    expect(loads, 2);
    expect(modules.modules.single.id, testModule.id);
    await controller.toggleInspector();
    repository.modules = [];
    await controller.toggleInspector();
    await Future<void>.delayed(Duration.zero);
    expect(loads, 3);
    expect(modules.modules, isEmpty);
    await controller.close();
    await Future<void>.delayed(const Duration(milliseconds: 10));
    expect(loads, 3);
  });

  testWidgets('Quick Paste exposes repeated copy, pin and delete failures', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    await controller.initialize();
    await controller.opened(1);
    await tester.binding.setSurfaceSize(const Size(448, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    controller.focus(controller.items.first);
    repository.failCopy = true;
    await controller.activateFocused();
    await tester.pumpAndSettle();
    expect(find.text('Action failed'), findsOneWidget);
    expect(find.text('This clip could not be copied.'), findsOneWidget);
    expect(host.closeIds, isEmpty);
    await tester.pump(const Duration(seconds: 6));
    await tester.pumpAndSettle();
    repository.failPin = true;
    await controller.toggleFocusedPin();
    await tester.pumpAndSettle();
    expect(find.text('The pin could not be updated.'), findsOneWidget);
    await tester.pump(const Duration(seconds: 6));
    await tester.pumpAndSettle();
    repository.failDelete = true;
    await controller.deleteFocused();
    await tester.pumpAndSettle();
    expect(find.text('This clip could not be deleted.'), findsOneWidget);
    expect(controller.items, hasLength(2));
    await tester.pump(const Duration(seconds: 6));
    await tester.pumpAndSettle();
    await controller.deleteFocused();
    await tester.pumpAndSettle();
    expect(find.text('This clip could not be deleted.'), findsOneWidget);
    await tester.pump(const Duration(seconds: 6));
    await tester.pumpAndSettle();
    await tester.pumpWidget(const SizedBox.shrink());
  });

  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.linux,
  ]) {
    testWidgets(
      'standard find, text editing, and close in Quick Paste on $platform',
      (tester) async {
        var clipboard = '';
        final messenger =
            TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
        messenger.setMockMethodCallHandler(SystemChannels.platform, (
          call,
        ) async {
          if (call.method == 'Clipboard.setData') {
            clipboard = (call.arguments as Map)['text'] as String;
          }
          if (call.method == 'Clipboard.getData') return {'text': clipboard};
          return null;
        });
        addTearDown(
          () =>
              messenger.setMockMethodCallHandler(SystemChannels.platform, null),
        );
        final repository = _Repository();
        final host = _ContextHost();
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
        );
        await tester.binding.setSurfaceSize(const Size(448, 800));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        await tester.pumpWidget(QuickPasteApp(controller: controller));
        await tester.pumpAndSettle();
        await controller.opened(1);
        await tester.pumpAndSettle();
        final search = find.byType(EditableText).first;
        await tester.enterText(search, 'find text');
        await tester.pumpAndSettle();
        await tester.pump(const Duration(milliseconds: 600));
        final modifier = platform == TargetPlatform.macOS
            ? LogicalKeyboardKey.metaLeft
            : LogicalKeyboardKey.controlLeft;
        await tester.sendKeyDownEvent(modifier);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyF);
        await tester.sendKeyUpEvent(modifier);
        await tester.pumpAndSettle();
        final editable = tester.widget<EditableText>(search);
        expect(editable.focusNode.hasFocus, isTrue);
        expect(
          editable.controller.selection,
          const TextSelection(baseOffset: 0, extentOffset: 9),
        );
        await tester.sendKeyDownEvent(modifier);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyX);
        await tester.sendKeyUpEvent(modifier);
        await tester.pumpAndSettle();
        expect(editable.controller.text, isEmpty);
        expect(repository.copied, isEmpty);
        expect(host.closeCalls, 0);
        await tester.pump(const Duration(milliseconds: 600));
        await tester.sendKeyDownEvent(modifier);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyZ);
        await tester.sendKeyUpEvent(modifier);
        await tester.pumpAndSettle();
        expect(editable.controller.text, 'find text');
        await tester.sendKeyDownEvent(modifier);
        if (platform != TargetPlatform.windows) {
          await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
        }
        await tester.sendKeyEvent(
          platform == TargetPlatform.windows
              ? LogicalKeyboardKey.keyY
              : LogicalKeyboardKey.keyZ,
        );
        if (platform != TargetPlatform.windows) {
          await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
        }
        await tester.sendKeyUpEvent(modifier);
        await tester.pumpAndSettle();
        expect(editable.controller.text, isEmpty);
        await tester.sendKeyDownEvent(modifier);
        await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
        if (platform == TargetPlatform.macOS) {
          await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
        }
        await tester.sendKeyEvent(LogicalKeyboardKey.keyV);
        if (platform == TargetPlatform.macOS) {
          await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
        }
        await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
        await tester.sendKeyUpEvent(modifier);
        await tester.pumpAndSettle();
        expect(editable.controller.text, 'find text');
        expect(repository.copied, isEmpty);
        await tester.sendKeyDownEvent(modifier);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyW);
        await tester.sendKeyUpEvent(modifier);
        await tester.pumpAndSettle();
        expect(host.closeIds, [1]);
        await controller.opened(2);
        await tester.pumpAndSettle();
        await tester.sendKeyDownEvent(modifier);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyQ);
        await tester.sendKeyUpEvent(modifier);
        await tester.pumpAndSettle();
        expect(host.quitCalls, 1);
        await tester.pumpWidget(const SizedBox.shrink());
      },
      variant: TargetPlatformVariant({platform}),
    );
  }
  test(
    'file transfer reports written bytes, reuses a copy, and retries failure',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'copypaste-transfer-test-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final repository = _Repository()..savedBytes = [1, 2, 3, 4];
      final transfer = HistoryFileTransfer(
        repository,
        temporaryDirectory: () async => directory,
      );
      final clip = HistoryClip(
        id: 'remote-file',
        contentType: 'file',
        preview: '/remote/report.bin',
        createdAt: DateTime.utc(2026),
        pinned: false,
        file: const HistoryFileDetails(name: 'report.bin', sizeBytes: 4),
      );
      repository.failSave = true;
      await expectLater(
        transfer.prepare(clip, onProgress: (_) {}),
        throwsStateError,
      );
      repository.failSave = false;
      repository.saveGate = Completer<void>();
      final progress = <double>[];
      final halfWritten = Completer<void>();
      final preparation = transfer.prepare(
        clip,
        onProgress: (value) {
          progress.add(value);
          if (value == 0.5 && !halfWritten.isCompleted) halfWritten.complete();
        },
      );
      await halfWritten.future;
      expect(progress, contains(0.5));
      repository.saveGate!.complete();
      final path = await preparation;
      repository.saveGate = null;
      expect(await File(path).readAsBytes(), [1, 2, 3, 4]);
      expect(progress.first, 0);
      expect(progress.last, 1);
      expect(await transfer.prepare(clip, onProgress: progress.add), path);
      expect(repository.saved.length, 1);
      repository.saveGate = Completer<void>();
      var cancelled = false;
      final destination = File('${directory.path}/cancelled.bin');
      final cancellableWriteStarted = Completer<void>();
      final cancelledSave = transfer.saveTo(
        clip,
        destination.path,
        onProgress: (value) {
          if (value == 0.5 && !cancellableWriteStarted.isCompleted) {
            cancellableWriteStarted.complete();
          }
        },
        isCancelled: () => cancelled,
      );
      await cancellableWriteStarted.future;
      cancelled = true;
      repository.saveGate!.complete();
      await expectLater(
        cancelledSave,
        throwsA(isA<HistoryFileTransferCancelled>()),
      );
      expect(await destination.exists(), isFalse);
    },
  );

  test('remote file waits for one transfer before copying', () async {
    final directory = await Directory.systemTemp.createTemp(
      'copypaste-transfer-action-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final repository = _Repository()
      ..savedBytes = [1, 2, 3, 4]
      ..saveGate = Completer<void>();
    final clip = HistoryClip(
      id: 'remote-file',
      contentType: 'file',
      preview: '/source/report.bin',
      createdAt: DateTime.utc(2026),
      pinned: false,
      file: const HistoryFileDetails(name: 'report.bin', sizeBytes: 4),
    );
    repository.clips
      ..clear()
      ..add(clip);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
      fileTransfer: HistoryFileTransfer(
        repository,
        temporaryDirectory: () async => directory,
      ),
    );
    await controller.initialize();
    await controller.opened(1);
    final activation = controller.activate(clip);
    await repository.saveStarted.future;
    expect(repository.copied, isEmpty);
    expect(controller.activating, isFalse);
    expect(controller.transferProgressFor(clip.id), isNotNull);
    expect(controller.history.isDownloadPending(clip.id), isTrue);
    expect(
      controller.history.downloadProgressFor(clip.id),
      controller.transferProgressFor(clip.id),
    );
    await controller.activate(clip);
    expect(repository.saved.length, 1);
    repository.saveGate!.complete();
    await activation;
    expect(repository.copied, [clip.id]);
    controller.dispose();
  });

  test(
    'Quick Paste offers retry after a failed remote file transfer',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'copypaste-transfer-retry-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final repository = _Repository()
        ..savedBytes = [1, 2]
        ..failSave = true;
      final clip = HistoryClip(
        id: 'retry-file',
        contentType: 'file',
        preview: '/source/retry.bin',
        createdAt: DateTime.utc(2026),
        pinned: false,
        file: const HistoryFileDetails(name: 'retry.bin', sizeBytes: 2),
      );
      repository.clips
        ..clear()
        ..add(clip);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
        fileTransfer: HistoryFileTransfer(
          repository,
          temporaryDirectory: () async => directory,
        ),
      );
      await controller.initialize();
      await controller.opened(1);
      await controller.activate(clip);
      expect(controller.transferFailedFor(clip.id), isTrue);
      expect(controller.history.downloadErrorFor(clip.id), isNotEmpty);
      expect(repository.copied, isEmpty);
      repository.failSave = false;
      await controller.activate(clip);
      expect(controller.transferFailedFor(clip.id), isFalse);
      expect(repository.copied, [clip.id]);
      controller.dispose();
    },
  );

  test('background file transfer leaves other clips usable and cannot paste after another action', () async {
    final repository = _Repository();
    final file = HistoryClip(
      id: 'background-file',
      contentType: 'file',
      preview: 'file.bin',
      createdAt: DateTime.utc(2026),
      pinned: false,
      file: const HistoryFileDetails(name: 'file.bin', sizeBytes: 4),
    );
    final text = HistoryClip(
      id: 'other-text',
      contentType: 'text',
      preview: 'Other text',
      createdAt: DateTime.utc(2026),
      pinned: false,
    );
    repository.clips
      ..clear()
      ..addAll([file, text]);
    final transfer = _WaitingFileTransfer(repository);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
      fileTransfer: transfer,
    );
    await controller.initialize();
    await controller.opened(1);
    final pending = controller.activate(file);
    await Future<void>.delayed(Duration.zero);
    expect(controller.activating, isFalse);
    expect(controller.history.isDownloadPending(file.id), isTrue);
    await controller.activate(text);
    expect(repository.copied, [text.id]);
    transfer.finish.complete('/downloaded/file.bin');
    await pending;
    expect(repository.copied, [text.id]);
    controller.dispose();
  });

  testWidgets(
    'Quick Paste explains transfer failure above metadata and separates the shortcut',
    (tester) async {
      final repository = _Repository();
      final clip = HistoryClip(
        id: 'failed-file',
        contentType: 'file',
        preview: '/source/report.bin',
        createdAt: DateTime.utc(2026),
        pinned: false,
        file: const HistoryFileDetails(name: 'report.bin', sizeBytes: 4),
      );
      repository.clips
        ..clear()
        ..add(clip);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
        fileTransfer: _FailedFileTransfer(repository),
        fileDownloader: _Downloader(),
      );
      await tester.binding.setSurfaceSize(const Size(960, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      await controller.opened(1, inspectorVisible: true);
      await tester.pumpAndSettle();
      await controller.activate(clip);
      await tester.pumpAndSettle();
      final reason = find.text(
        'The source device is offline. Bring it online and retry.',
      );
      expect(reason, findsOneWidget);
      expect(
        tester.getTopLeft(reason).dy,
        lessThan(
          tester
              .getTopLeft(find.byKey(const ValueKey('history-detail-metadata')))
              .dy,
        ),
      );
      final row = find.byKey(const ValueKey('quick-paste-failed-file'));
      final shortcut = find.descendant(
        of: row,
        matching: find.byType(MenuShortcut),
      );
      expect(
        tester.getRect(shortcut).left -
            tester.getRect(find.text('Failed')).right,
        greaterThanOrEqualTo(AppSpacing.sm),
      );
      expect(find.text('Retry'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
    variant: TargetPlatformVariant({TargetPlatform.macOS}),
  );

  testWidgets('file clip shows its name and a one-line middle-ellipsized path', (
    tester,
  ) async {
    final repository = _Repository();
    final path =
        '/Users/source/Documents/${List.filled(12, 'nested/').join()}report.bin';
    final clip = HistoryClip(
      id: 'remote-file',
      contentType: 'file',
      preview: path,
      createdAt: DateTime.utc(2026),
      pinned: false,
      file: HistoryFileDetails(
        name: 'report.bin',
        sourceReference: path,
        sizeBytes: 4,
      ),
    );
    repository.clips
      ..clear()
      ..add(clip);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
    );
    await tester.binding.setSurfaceSize(const Size(448, 600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    await controller.opened(1);
    await tester.pumpAndSettle();
    expect(find.text('report.bin'), findsOneWidget);
    final rendered = tester.widget<Text>(
      find.byKey(const ValueKey('quick-paste-file-path-remote-file')),
    );
    expect(rendered.maxLines, 1);
    expect(rendered.data, startsWith('/Users'));
    expect(rendered.data, endsWith('report.bin'));
    expect(rendered.data, contains('…'));

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('middle ellipsis keeps both ends within the measured width', (
    tester,
  ) async {
    const style = TextStyle(fontSize: 12);
    final shortened = middleEllipsis(
      '/Users/source/Documents/deep/folder/report.bin',
      maxWidth: 240,
      style: style,
      direction: TextDirection.ltr,
    );
    expect(shortened, startsWith('/Users'));
    expect(shortened, endsWith('report.bin'));
    expect(shortened, contains('…'));
    final rtl = middleEllipsis(
      '😀אבגדהוזחטיכלמנסעפצקרשת📄',
      maxWidth: 100,
      style: style,
      direction: TextDirection.rtl,
    );
    expect(rtl, startsWith('😀'));
    expect(rtl, endsWith('📄'));
    expect(rtl, contains('…'));
    expect(
      middleEllipsis(
        'anything',
        maxWidth: 0,
        style: style,
        direction: TextDirection.ltr,
      ),
      isEmpty,
    );
    expect(
      middleEllipsis(
        'short',
        maxWidth: 240,
        style: style,
        direction: TextDirection.ltr,
      ),
      'short',
    );
  });

  testWidgets(
    'native reveal waits for the visible image and its rendered frame',
    (tester) async {
      final repository = _Repository()..imageLoadGate = Completer<void>();
      repository.clips.insert(
        0,
        HistoryClip(
          id: 'image',
          contentType: 'image/png',
          preview: '[image]',
          createdAt: DateTime.utc(2026),
          pinned: false,
        ),
      );
      final host = _FrameHost()..openWhenReady = true;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      await tester.binding.setSurfaceSize(const Size(448, 600));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pump();
      await tester.pump();
      expect(repository.imagePreviewIds, contains('image'));
      expect(host.presentedIds, isEmpty);
      repository.imageLoadGate!.complete();
      await tester.pump();
      await tester.runAsync(() async {
        final preview = await controller.history.requestImagePreview(
          'image',
          maxEdge: 480,
        );
        await precacheImage(
          MemoryImage(preview!.bytes),
          tester.element(find.byType(QuickPasteApp)),
        );
      });
      await tester.pumpAndSettle();
      expect(host.presentedIds, [1]);
      expect(find.text('[image]'), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('a superseded media load cannot reveal an older presentation', (
    tester,
  ) async {
    final repository = _Repository()..imageLoadGate = Completer<void>();
    repository.clips.insert(
      0,
      HistoryClip(
        id: 'image',
        contentType: 'image/png',
        preview: '[image]',
        createdAt: DateTime.utc(2026),
        pinned: false,
      ),
    );
    final host = _FrameHost()..openWhenReady = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    await tester.binding.setSurfaceSize(const Size(448, 600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pump();
    await tester.pump();
    await controller.opened(2);
    await tester.pump();
    repository.imageLoadGate!.complete();
    await tester.pump();
    await tester.runAsync(() async {
      final preview = await controller.history.requestImagePreview(
        'image',
        maxEdge: 480,
      );
      await precacheImage(
        MemoryImage(preview!.bytes),
        tester.element(find.byType(QuickPasteApp)),
      );
    });
    await tester.pumpAndSettle();
    expect(host.presentedIds, [2]);
    expect(tester.takeException(), isNull);
  });

  testWidgets('offscreen images do not delay the first visible menu frame', (
    tester,
  ) async {
    final repository = _Repository()..imageLoadGate = Completer<void>();
    _populateHistory(repository, 40);
    repository.clips.add(
      HistoryClip(
        id: 'image',
        contentType: 'image/png',
        preview: '[image]',
        createdAt: DateTime.utc(2026),
        pinned: false,
      ),
    );
    final host = _FrameHost()..openWhenReady = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    await tester.binding.setSurfaceSize(const Size(448, 600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    expect(host.presentedIds, [1]);
    expect(repository.imagePreviewIds, isNot(contains('image')));
    expect(repository.imageLoadGate!.isCompleted, isFalse);
    repository.imageLoadGate!.complete();
  });
  test(
    'first presentation reuses initial history without filter queries',
    () async {
      final repository = _Repository();
      final host = _ContextHost()..openWhenReady = true;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      expect(repository.queryCalls, 1);
      expect(repository.facetsCalls, 0);
      expect(controller.items, hasLength(2));
      await controller.opened(2);
      expect(repository.queryCalls, 2);
    },
  );

  for (final initiallyVisible in [false, true]) {
    test(
      'inspector resources load on demand without blocking presentation ($initiallyVisible)',
      () async {
        final pending = Completer<void>();
        var preparations = 0;
        final host = _ContextHost()
          ..openWhenReady = true
          ..initialInspectorVisible = initiallyVisible;
        final controller = QuickPasteController(
          repository: _Repository(),
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
          initializeInspector: () {
            preparations += 1;
            return pending.future;
          },
        );
        addTearDown(controller.dispose);
        await controller.initialize();
        expect(controller.items, hasLength(2));
        expect(preparations, initiallyVisible ? 1 : 0);
        if (!initiallyVisible) await controller.toggleInspector();
        expect(preparations, 1);
        expect(controller.inspectorOpen, isTrue);
        await controller.toggleInspector();
        await controller.toggleInspector();
        expect(preparations, 1);
        expect(pending.isCompleted, isFalse);
        pending.complete();
      },
    );
  }

  testWidgets(
    'Quick Paste spoiler reveal neither copies nor persists into a reopened menu',
    (tester) async {
      final repository = _Repository();
      repository.clips.clear();
      repository.clips.add(
        HistoryClip(
          id: 'secret-menu',
          contentType: 'text',
          preview: 'SYNTHETIC MENU SECRET',
          body: 'SYNTHETIC MENU SECRET',
          secret: true,
          createdAt: DateTime.utc(2026),
          pinned: false,
        ),
      );
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
      );
      await controller.initialize();
      await controller.opened(1);
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pump();
      expect(find.text('SYNTHETIC MENU SECRET').hitTestable(), findsNothing);
      final generation = controller.presentationGeneration;
      final quickSpoiler = find.byKey(
        ValueKey('quick-spoiler-secret-menu-$generation'),
      );
      final quickBody = find.descendant(
        of: quickSpoiler,
        matching: find.text('SYNTHETIC MENU SECRET'),
      );
      expect(tester.getSize(quickSpoiler), tester.getSize(quickBody));
      await tester.tap(
        find.descendant(of: quickSpoiler, matching: find.byType(Button)),
      );
      await tester.pump();
      await tester.pump(AppMotion.spoilerReveal);
      expect(find.text('SYNTHETIC MENU SECRET').hitTestable(), findsOneWidget);
      expect(repository.copied, isEmpty);
      await controller.opened(2);
      await tester.pump();
      expect(find.text('SYNTHETIC MENU SECRET').hitTestable(), findsNothing);
      expect(repository.copied, isEmpty);
      await controller.toggleInspector();
      await tester.pump();
      final inspectorGeneration = controller.presentationGeneration;
      await tester.tap(
        find.descendant(
          of: find.byKey(
            ValueKey('inspector-spoiler-secret-menu-$inspectorGeneration'),
          ),
          matching: find.byType(Button),
        ),
      );
      await tester.pump();
      await tester.pump(AppMotion.spoilerReveal);
      expect(
        find.text('SYNTHETIC MENU SECRET').hitTestable(),
        findsNWidgets(2),
      );
      await controller.opened(3);
      if (!controller.inspectorOpen) {
        await controller.toggleInspector();
      }
      await tester.pump();
      expect(find.text('SYNTHETIC MENU SECRET').hitTestable(), findsNothing);
      expect(repository.copied, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  for (final scale in [1.0, 2.0]) {
    testWidgets(
      'spoiler retains the ordinary menu row height at scale $scale',
      (tester) async {
        final repository = _Repository();
        repository.clips[0] = repository.clips[0].copyWith(
          secret: true,
          pinned: false,
        );
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: _ContextHost(),
        );
        tester.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        await tester.binding.setSurfaceSize(const Size(816, 800));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        await controller.initialize();
        await controller.opened(1);
        await tester.pumpWidget(QuickPasteApp(controller: controller));
        await tester.pump();
        final secret = find.byKey(const ValueKey('quick-paste-pinned'));
        final ordinary = find.byKey(const ValueKey('quick-paste-recent'));
        final height = tester.getSize(ordinary).height;
        expect(tester.getSize(secret).height, closeTo(height, 0.01));
        await tester.tap(
          find.descendant(of: secret, matching: find.byType(Button)).last,
        );
        await tester.pump();
        await tester.pump(AppMotion.spoilerReveal);
        expect(tester.getSize(secret).height, closeTo(height, 0.01));
        expect(repository.copied, isEmpty);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.android,
  ]) {
    testWidgets(
      'inspector keeps scroll positions and menu geometry on $platform',
      (tester) async {
        final repository = _Repository();
        _populateHistory(repository, 40);
        repository.clips.addAll(
          List.generate(
            15,
            (index) => HistoryClip(
              id: 'pin-$index',
              contentType: 'text',
              preview: 'Pinned $index',
              createdAt: DateTime.utc(2026),
              pinned: true,
            ),
          ),
        );
        final host = _ContextHost();
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
        );
        await tester.binding.setSurfaceSize(const Size(448, 800));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        await controller.initialize();
        await controller.opened(1);
        await tester.pumpWidget(QuickPasteApp(controller: controller));
        await tester.pumpAndSettle();
        final recentFinder = find.byKey(
          const ValueKey('quick-paste-history-scroll'),
        );
        final pinnedFinder = find.byKey(
          const ValueKey('quick-paste-pinned-scroll'),
        );
        final recent = tester.widget<ScrollView>(recentFinder).controller!;
        final pinned = tester.widget<ScrollView>(pinnedFinder).controller!;
        recent.jumpTo(400);
        pinned.jumpTo(80);
        await tester.pumpAndSettle();
        final recentPosition = recent.position;
        final pinnedPosition = pinned.position;
        final headerWidth = tester
            .getSize(find.byKey(const ValueKey('quick-paste-header')))
            .width;
        host.inspectorGate = Completer<void>();
        final opening = controller.toggleInspector();
        await tester.pump();
        // Model native resize arriving before its method-channel reply.
        await tester.binding.setSurfaceSize(const Size(816, 800));
        await tester.pump();
        expect(
          tester
              .getSize(find.byKey(const ValueKey('quick-paste-header')))
              .width,
          headerWidth,
        );
        host.inspectorGate!.complete();
        await opening;
        await tester.pumpAndSettle();
        expect(recent.position, same(recentPosition));
        expect(pinned.position, same(pinnedPosition));
        expect(recent.offset, 400);
        expect(pinned.offset, 80);
        expect(
          tester
              .getSize(find.byKey(const ValueKey('quick-paste-header')))
              .width,
          headerWidth,
        );
        host.inspectorGate = null;
        await controller.toggleInspector();
        await tester.binding.setSurfaceSize(const Size(448, 800));
        await tester.pumpAndSettle();
        expect(recent.position, same(recentPosition));
        expect(pinned.position, same(pinnedPosition));
        expect(recent.offset, 400);
        expect(pinned.offset, 80);
        for (final label in ['Clear', 'Preferences…', 'About', 'Quit']) {
          final buttons = find.ancestor(
            of: find.text(label),
            matching: find.byType(Button),
          );
          final button = tester.widget<Button>(buttons.first);
          for (final states in [
            <WidgetState>{},
            {WidgetState.hovered},
            {WidgetState.focused},
            {WidgetState.pressed},
          ]) {
            final decoration = button.style.decoration(
              tester.element(buttons.first),
              states,
            ) as BoxDecoration;
            expect(decoration.border?.top.style, BorderStyle.none);
          }
        }
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
      variant: TargetPlatformVariant({platform}),
    );
  }

  test('cold engine restores the native inspector state', () async {
    final host = _ContextHost()
      ..openWhenReady = true
      ..initialInspectorVisible = true;
    final controller = QuickPasteController(
      repository: _Repository(),
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    expect(controller.inspectorOpen, isTrue);
    expect(controller.history.selectedClip?.id, 'recent');
  });

  test('cold engine opening waits until its controller is ready', () async {
    final host = _ContextHost()..openWhenReady = true;
    final controller = QuickPasteController(
      repository: _Repository(),
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    expect(host.readyCalls, 1);
    expect(controller.presentationGeneration, 1);
    await controller.activateIndex(0);
    expect(host.closeIds, [1]);
  });

  test(
    'shutdown waits for the owned repository before releasing its engine',
    () async {
      final disposed = Completer<void>();
      var disposals = 0;
      final host = _ContextHost();
      final controller = QuickPasteController(
        repository: _Repository(),
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
        disposeRepository: () {
          disposals += 1;
          return disposed.future;
        },
      );
      await controller.initialize();
      var completed = false;
      final shutdown = host.shutdownHandler!().then((_) => completed = true);
      await Future<void>.delayed(Duration.zero);
      expect(completed, isFalse);
      expect(host.openedHandler, isNull);
      disposed.complete();
      await shutdown;
      await controller.shutdown();
      expect(completed, isTrue);
      expect(disposals, 1);
    },
  );

  testWidgets(
    'loads history pages on scroll and builds only nearby rows',
    (tester) async {
      final repository = _PagedRepository();
      _populateHistory(repository, 125);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
      );
      await tester.binding.setSurfaceSize(const Size(448, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();

      final historyScroll = find.byKey(
        const ValueKey('quick-paste-history-scroll'),
      );
      ScrollController scroll() =>
          tester.widget<ScrollView>(historyScroll).controller!;
      expect(controller.items, hasLength(50));
      expect(repository.requests, hasLength(1));
      expect(find.byKey(const ValueKey('quick-paste-clip-49')), findsNothing);
      scroll().jumpTo(scroll().position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(100));
      expect(repository.requests.map((request) => request.cursor), [
        null,
        '50',
      ]);
      scroll().jumpTo(scroll().position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(125));
      expect(controller.items.map((clip) => clip.id).toSet(), hasLength(125));
      expect(
        repository.requests.map((request) => request.limit),
        everyElement(50),
      );
      expect(controller.history.canLoadMore, isFalse);
      scroll().jumpTo(scroll().position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey('quick-paste-clip-124')),
        findsOneWidget,
      );
      expect(repository.requests, hasLength(3));

      await controller.search('Clip 1');
      await tester.pumpAndSettle();
      expect(scroll().offset, 0);
      expect(controller.items, hasLength(36));
      expect(repository.requests.last.cursor, isNull);
      expect(repository.requests.last.search, 'Clip 1');
      await controller.opened(1);
      await tester.pumpAndSettle();
      expect(scroll().offset, 0);
      expect(controller.items, hasLength(50));
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets('keeps pinned and recent windows independently bounded', (
    tester,
  ) async {
    final repository = _PagedRepository();
    _populateHistory(repository, 280, pinnedCount: 80);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
    );
    await tester.binding.setSurfaceSize(const Size(448, 500));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    Future<void> loadPage() async {
      final load = controller.history.loadMore();
      await tester.pumpAndSettle();
      await load;
      await tester.pumpAndSettle();
    }

    for (var page = 1; page < 4; page++) {
      await loadPage();
    }
    final pinned = find.byKey(const ValueKey('quick-paste-clip-0'));
    final recent = find.byKey(const ValueKey('quick-paste-clip-80'));
    expect(pinned, findsOneWidget);
    expect(recent, findsOneWidget);
    final pinnedY = tester.getTopLeft(pinned).dy;
    final recentY = tester.getTopLeft(recent).dy;
    await loadPage();
    expect(pinned, findsOneWidget);
    expect(recent, findsOneWidget);
    expect(tester.getTopLeft(pinned).dy, closeTo(pinnedY, 1));
    expect(tester.getTopLeft(recent).dy, closeTo(recentY, 1));
    expect(controller.history.visitedIds.length, 250);
    expect(controller.items.where((clip) => clip.pinned), hasLength(80));
    expect(
      controller.items.where((clip) => !clip.pinned).length,
      lessThanOrEqualTo(200),
    );
    expect(
      controller.shortcuts.values.any((clip) => clip.id == 'clip-0'),
      isTrue,
    );
    expect(tester.takeException(), isNull);
  });

  for (final pinned in [false, true]) {
    testWidgets(
      'defers offscreen image previews (pinned: $pinned)',
      (tester) async {
        final repository = _PagedRepository();
        _populateHistory(
          repository,
          80,
          pinnedCount: pinned ? 80 : 0,
          images: true,
        );
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: _ContextHost(),
        );
        await tester.binding.setSurfaceSize(const Size(448, 500));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        await tester.pumpWidget(QuickPasteApp(controller: controller));
        await tester.pumpAndSettle();
        expect(controller.items, hasLength(50));
        expect(repository.imagePreviewIds.length, lessThan(50));
        expect(repository.imagePreviewIds, isNot(contains('clip-49')));
        expect(find.byKey(const ValueKey('quick-paste-clip-49')), findsNothing);
        expect(tester.takeException(), isNull);
      },
      variant: TargetPlatformVariant({
        TargetPlatform.macOS,
        TargetPlatform.windows,
        TargetPlatform.android,
      }),
    );
  }

  testWidgets(
    'paginates a pinned-only page into the recent history',
    (tester) async {
      final repository = _PagedRepository();
      _populateHistory(repository, 125, pinnedCount: 70);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
      );
      await tester.binding.setSurfaceSize(const Size(448, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(50));
      expect(controller.items.every((clip) => clip.pinned), isTrue);
      final pinnedScroll = tester
          .widget<ScrollView>(
            find.byKey(const ValueKey('quick-paste-pinned-scroll')),
          )
          .controller!;
      pinnedScroll.jumpTo(pinnedScroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(100));
      expect(find.byKey(const ValueKey('quick-paste-clip-70')), findsOneWidget);
      final recentScroll = tester
          .widget<ScrollView>(
            find.byKey(const ValueKey('quick-paste-history-scroll')),
          )
          .controller!;
      recentScroll.jumpTo(recentScroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(125));
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets(
    'keyboard traverses offscreen rows and page boundaries',
    (tester) async {
      final repository = _PagedRepository();
      _populateHistory(repository, 81, pinnedCount: 1);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
      );
      await tester.binding.setSurfaceSize(const Size(448, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      await controller.opened(1);
      await tester.pumpAndSettle();
      final pinned = find.byKey(const ValueKey('quick-paste-clip-0'));
      final pinnedRect = tester.getRect(pinned);
      for (var page = 0; page < 12; page++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.pageDown);
        await tester.pumpAndSettle();
      }
      expect(controller.focusedClip?.id, 'clip-60');
      expect(controller.items, hasLength(81));
      final focused = find.byKey(const ValueKey('quick-paste-clip-60'));
      expect(focused, findsOneWidget);
      final historyRect = tester.getRect(
        find.byKey(const ValueKey('quick-paste-history-scroll')),
      );
      final focusedRect = tester.getRect(focused);
      expect(focusedRect.top, greaterThanOrEqualTo(historyRect.top));
      expect(focusedRect.bottom, lessThanOrEqualTo(historyRect.bottom));
      expect(tester.getRect(pinned), pinnedRect);
      await tester.sendKeyEvent(LogicalKeyboardKey.pageUp);
      await tester.pumpAndSettle();
      expect(controller.focusedClip?.id, 'clip-55');
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
      await tester.pumpAndSettle();
      expect(controller.focusedClip?.id, 'clip-54');
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('quick-paste-search')))
            .focusNode
            ?.hasFocus,
        isTrue,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(repository.copied, ['clip-54']);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets(
    'failed pages wait for retry and can continue paginating',
    (tester) async {
      final repository = _PagedRepository()..failNextPage = true;
      _populateHistory(repository, 125);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
      );
      await tester.binding.setSurfaceSize(const Size(448, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      final historyScroll = find.byKey(
        const ValueKey('quick-paste-history-scroll'),
      );
      final scroll = tester.widget<ScrollView>(historyScroll).controller!;
      scroll.jumpTo(scroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(50));
      expect(repository.requests, hasLength(2));
      expect(controller.paginationFailed, isTrue);
      await tester.pumpAndSettle();
      expect(repository.requests, hasLength(2));
      await tester.tap(find.text('Try again'));
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(100));
      expect(controller.paginationFailed, isFalse);
      scroll.jumpTo(scroll.position.maxScrollExtent);
      await tester.pumpAndSettle();
      expect(controller.items, hasLength(125));
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  test('scroll and keyboard share the pending page request', () async {
    final repository = _PagedRepository();
    _populateHistory(repository, 125);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.focus(controller.items.last);
    repository.pageGate = Completer<void>();
    final page = controller.loadMore();
    await repository.pageStarted.future;
    final navigation = controller.moveFocus(1);
    expect(repository.requests, hasLength(2));
    repository.pageGate!.complete();
    await Future.wait([page, navigation]);
    expect(repository.requests, hasLength(2));
    expect(controller.items, hasLength(100));
    expect(controller.focusedClip?.id, 'clip-50');
  });

  test('page navigation loads only the final inspector selection', () async {
    final repository = _PagedRepository();
    _populateHistory(repository, 125);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1, inspectorVisible: true);
    expect(repository.getCalls, 1);
    await controller.moveFocus(5);
    expect(controller.focusedClip?.id, 'clip-4');
    expect(controller.history.selectedClip?.id, 'clip-4');
    expect(repository.getCalls, 2);
  });

  test('search cancels pending keyboard pagination', () async {
    final repository = _PagedRepository();
    _populateHistory(repository, 125);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost(),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.focus(controller.items.last);
    repository.pageGate = Completer<void>();
    final navigation = controller.moveFocus(1);
    await repository.pageStarted.future;
    await controller.search('Clip 124');
    repository.pageGate!.complete();
    await navigation;
    expect(controller.items.map((clip) => clip.id), ['clip-124']);
    expect(controller.focusedClip, isNull);
    expect(controller.paginationFailed, isFalse);
  });

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
    expect(
      search.left -
          tester.getRect(find.byKey(const ValueKey('quick-paste-home'))).right,
      AppSpacing.xs,
    );
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

  for (final kind in HistoryClipKind.values) {
    testWidgets('tray-sized Quick Paste renders ${kind.name} content', (
      tester,
    ) async {
      final repository = _Repository();
      final isImage = kind == HistoryClipKind.image;
      final id = isImage ? 'image' : kind.name;
      final label = kind == HistoryClipKind.file
          ? 'invoice.pdf'
          : 'Preview for ${kind.name}';
      repository.clips
        ..clear()
        ..add(
          HistoryClip(
            id: id,
            contentType: isImage
                ? 'image/png'
                : kind == HistoryClipKind.file
                ? 'file'
                : 'text/plain',
            kind: kind,
            preview: isImage ? '[image]' : label,
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        );
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost()..permissionGranted = true,
      );
      await tester.binding.setSurfaceSize(const Size(448, 600));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();

      if (isImage) {
        final preview = find.descendant(
          of: find.byKey(const ValueKey('quick-paste-image')),
          matching: find.byType(Image),
        );
        expect(preview, findsOneWidget);
        expect(tester.widget<Image>(preview).image, isA<ResizeImage>());
        expect(find.text('[image]'), findsNothing);
      } else if (kind == HistoryClipKind.file) {
        expect(find.text(label).hitTestable(), findsOneWidget);
        expect(find.text('Path unavailable').hitTestable(), findsOneWidget);
      } else {
        expect(find.text(label).hitTestable(), findsOneWidget);
      }
      expect(
        find.byKey(const ValueKey('quick-paste-search')).hitTestable(),
        findsOneWidget,
      );
      expect(find.text('Quit').hitTestable(), findsOneWidget);
      expect(tester.getRect(find.text('Quit')).bottom, lessThanOrEqualTo(600));
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('keeps pinned clips fixed below scrolling history', (
    tester,
  ) async {
    final repository = _Repository();
    repository.clips.addAll([
      for (var index = 0; index < 40; index++)
        HistoryClip(
          id: 'recent-$index',
          contentType: 'text/plain',
          preview: 'Recent clip $index',
          createdAt: DateTime.utc(2026),
          pinned: false,
        ),
    ]);
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: _ContextHost()..permissionGranted = true,
    );
    await tester.binding.setSurfaceSize(const Size(448, 500));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();

    final historyScroll = find.byKey(
      const ValueKey('quick-paste-history-scroll'),
    );
    final pinned = find.byKey(const ValueKey('quick-paste-pinned'));
    final pinnedRect = tester.getRect(pinned);
    final footerRect = tester.getRect(find.text('Clear'));
    expect(find.descendant(of: historyScroll, matching: pinned), findsNothing);
    expect(
      pinnedRect.top,
      greaterThanOrEqualTo(tester.getRect(historyScroll).bottom),
    );
    expect(pinnedRect.bottom, lessThan(footerRect.top));

    await tester.drag(historyScroll, const Offset(0, -300));
    await tester.pumpAndSettle();
    final scroll = tester.widget<ScrollView>(historyScroll);
    expect(scroll.controller!.offset, greaterThan(0));
    expect(tester.getRect(pinned), pinnedRect);
    expect(tester.getRect(find.text('Clear')), footerRect);
    expect(tester.takeException(), isNull);
  });

  for (final recentCount in [0, 40]) {
    testWidgets('fits many pinned clips with $recentCount recent clips', (
      tester,
    ) async {
      final repository = _Repository()..clips.clear();
      repository.clips.addAll([
        for (var index = 0; index < 40 + recentCount; index++)
          HistoryClip(
            id: 'clip-$index',
            contentType: 'text/plain',
            preview: 'Clip $index',
            createdAt: DateTime.utc(2026),
            pinned: index < 40,
          ),
      ]);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost()..permissionGranted = true,
      );
      await tester.binding.setSurfaceSize(const Size(448, 500));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      final pinnedScroll = find.byKey(
        const ValueKey('quick-paste-pinned-scroll'),
      );
      final footerRect = tester.getRect(find.text('Clear'));
      await tester.drag(pinnedScroll, const Offset(0, -300));
      await tester.pumpAndSettle();
      expect(tester.getRect(find.text('Clear')), footerRect);
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets(
    'keeps source icons stable across full-page rebuilds',
    (tester) async {
      final repository = _Repository()..clips.clear();
      repository.sourceIcon = HistorySourceAppIcon(
        Uint8List.fromList(image.encodePng(image.Image(width: 16, height: 16))),
      );
      repository.clips.addAll([
        for (var index = 0; index < 50; index++)
          HistoryClip(
            id: 'icon-$index',
            contentType: 'text/plain',
            preview: 'Clip $index',
            createdAt: DateTime.utc(2026),
            pinned: false,
            sourceApp: 'Editor',
            sourceAppIconId: 'app:editor',
          ),
      ]);
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost()..permissionGranted = true,
      );
      await tester.binding.setSurfaceSize(const Size(448, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      for (var frame = 0; frame < 12; frame++) {
        await tester.pump(const Duration(milliseconds: 100));
      }

      expect(repository.sourceIconCalls, 1);
      final iconFinder = find.descendant(
        of: find.byKey(const ValueKey('quick-paste-icon-0')),
        matching: find.byType(Avatar),
      );
      final provider = tester.widget<Avatar>(iconFinder).provider;
      expect(provider, isNotNull);
      controller.focus(controller.items[1]);
      for (var frame = 0; frame < 4; frame++) {
        await tester.pump(const Duration(milliseconds: 100));
        expect(tester.widget<Avatar>(iconFinder).provider, provider);
      }
      expect(repository.sourceIconCalls, 1);
      await controller.history.deleteClip('icon-0');
      await tester.pump(const Duration(milliseconds: 100));
      final nextIconFinder = find.descendant(
        of: find.byKey(const ValueKey('quick-paste-icon-1')),
        matching: find.byType(Avatar),
      );
      expect(tester.widget<Avatar>(nextIconFinder).provider, provider);
      expect(repository.sourceIconCalls, 1);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

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
        sourceAppIconId: 'app:editor',
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
    final ratio = tester.view.devicePixelRatio;
    expect(
      repository.imagePreviewBounds,
      contains((
        (240 * ratio).ceil().clamp(1, 480),
        (120 * ratio).ceil().clamp(1, 480),
      )),
    );
    final row = find.byKey(const ValueKey('quick-paste-image'));
    final size = tester.getSize(row);
    await controller.history.toggleSecret(repository.clips.first);
    await tester.pump();
    await tester.pump();
    expect(tester.getSize(row), size);
    expect(preview.hitTestable(), findsNothing);
    await controller.history.spoilers.setRevealed('image', true);
    await tester.pump();
    await tester.pump(AppMotion.spoilerReveal);
    expect(tester.getSize(row), size);
    expect(preview.hitTestable(), findsOneWidget);
    expect(repository.copied, isEmpty);
    expect(tester.takeException(), isNull);
  });

  for (final modifier in [
    LogicalKeyboardKey.controlLeft,
    LogicalKeyboardKey.controlRight,
  ]) {
    testWidgets(
      'Windows number accelerator uses top-row 1 from search with $modifier',
      (tester) async {
        final repository = _Repository();
        final host = _ContextHost()..permissionGranted = true;
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
        );
        await tester.binding.setSurfaceSize(const Size(448, 800));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        await tester.pumpWidget(QuickPasteApp(controller: controller));
        await tester.pumpAndSettle();
        await controller.opened(1);
        await tester.pumpAndSettle();
        final search = tester.widget<EditableText>(
          find.byType(EditableText).first,
        );
        expect(search.focusNode.hasFocus, isTrue);
        await tester.sendKeyDownEvent(modifier);
        await tester.sendKeyEvent(LogicalKeyboardKey.digit1);
        await tester.sendKeyUpEvent(modifier);
        await tester.pumpAndSettle();
        // The repository lists a pinned row first; 1 belongs to the recent row.
        expect(repository.copied, ['recent']);
        expect(host.pasteIds, [1]);
        expect(host.closeIds, isEmpty);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
      variant: TargetPlatformVariant({TargetPlatform.windows}),
    );
  }

  for (final (label, trigger, modifiers) in [
    ('numpad 1', LogicalKeyboardKey.numpad1, [LogicalKeyboardKey.controlLeft]),
    (
      'extra Shift',
      LogicalKeyboardKey.digit1,
      [LogicalKeyboardKey.controlLeft, LogicalKeyboardKey.shiftLeft],
    ),
    (
      'extra Alt',
      LogicalKeyboardKey.digit1,
      [LogicalKeyboardKey.controlLeft, LogicalKeyboardKey.altLeft],
    ),
  ]) {
    testWidgets(
      'Windows number accelerator ignores $label while search owns focus',
      (tester) async {
        final repository = _Repository();
        final host = _ContextHost()..permissionGranted = true;
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: MemoryQuickPastePreferencesStore(),
          host: host,
        );
        await tester.binding.setSurfaceSize(const Size(448, 800));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        await tester.pumpWidget(QuickPasteApp(controller: controller));
        await tester.pumpAndSettle();
        await controller.opened(1);
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<EditableText>(find.byType(EditableText).first)
              .focusNode
              .hasFocus,
          isTrue,
        );
        for (final modifier in modifiers) {
          await tester.sendKeyDownEvent(modifier);
        }
        await tester.sendKeyEvent(trigger);
        for (final modifier in modifiers.reversed) {
          await tester.sendKeyUpEvent(modifier);
        }
        await tester.pumpAndSettle();
        expect(repository.copied, isEmpty);
        expect(host.pasteIds, isEmpty);
        expect(host.closeIds, isEmpty);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
      variant: TargetPlatformVariant({TargetPlatform.windows}),
    );
  }

  testWidgets('Enter activates the newest clip on Linux without navigation', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost()..permissionGranted = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    await tester.binding.setSurfaceSize(const Size(448, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    await controller.opened(1);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(repository.copied, ['recent']);
    expect(host.pasteIds, [1]);
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant({TargetPlatform.linux}));

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

  testWidgets(
    'hover opens the inspector after the Maccy delay and follows selection',
    (tester) async {
      final repository = _Repository();
      final host = _ContextHost()..openWhenReady = true;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      await tester.binding.setSurfaceSize(const Size(816, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      addTearDown(mouse.removePointer);
      final recent = find.byKey(const ValueKey('quick-paste-recent'));
      final pinned = find.byKey(const ValueKey('quick-paste-pinned'));
      await mouse.moveTo(tester.getCenter(recent));
      await tester.pump(const Duration(milliseconds: 1499));
      expect(controller.focusedClip?.id, 'recent');
      expect(controller.inspectorOpen, isFalse);
      expect(repository.getCalls, 0);
      await tester.pump(const Duration(milliseconds: 1));
      await tester.pumpAndSettle();
      expect(host.inspectorChanges, [true]);
      expect(host.inspectorPresentationIds, [1]);
      expect(controller.inspectorOpen, isTrue);
      expect(controller.history.selectedClip?.id, 'recent');
      expect(
        find.byKey(const ValueKey('history-detail-inspector')),
        findsOneWidget,
      );
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('quick-paste-search')))
            .focusNode
            ?.hasFocus,
        isTrue,
      );
      await mouse.moveTo(tester.getCenter(pinned));
      await tester.pumpAndSettle();
      expect(controller.history.selectedClip?.id, 'pinned');
      expect(host.inspectorChanges, [true]);
      await mouse.moveTo(
        tester.getCenter(
          find.byKey(const ValueKey('history-detail-inspector')),
        ),
      );
      await tester.pump(const Duration(seconds: 2));
      expect(controller.inspectorOpen, isTrue);
      expect(repository.copied, isEmpty);
      expect(host.inspectorRemembered, [false]);
      await controller.close();
      await controller.opened(2);
      expect(controller.inspectorOpen, isFalse);
      await controller.toggleInspector();
      expect(host.inspectorRemembered, [false, true]);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

  testWidgets('leaving and changing hovered rows cancel the previous delay', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost()..openWhenReady = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    await tester.binding.setSurfaceSize(const Size(816, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);
    final recent = find.byKey(const ValueKey('quick-paste-recent'));
    final pinned = find.byKey(const ValueKey('quick-paste-pinned'));
    await mouse.moveTo(tester.getCenter(recent));
    await tester.pump(const Duration(milliseconds: 1000));
    await mouse.moveTo(
      tester.getCenter(find.byKey(const ValueKey('quick-paste-search'))),
    );
    await tester.pump(const Duration(seconds: 2));
    expect(controller.inspectorOpen, isFalse);
    expect(host.inspectorChanges, isEmpty);
    await mouse.moveTo(tester.getCenter(recent));
    await tester.pump(const Duration(milliseconds: 1000));
    await mouse.moveTo(tester.getCenter(pinned));
    await tester.pump(const Duration(milliseconds: 1000));
    expect(controller.inspectorOpen, isFalse);
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pumpAndSettle();
    expect(controller.history.selectedClip?.id, 'pinned');
    expect(repository.getCalls, 1);
    expect(host.inspectorChanges, [true]);
    expect(tester.takeException(), isNull);
  });

  testWidgets('scrolling cancels hover on a row that leaves the lazy list', (
    tester,
  ) async {
    final repository = _PagedRepository();
    _populateHistory(repository, 125);
    final host = _ContextHost()..openWhenReady = true;
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    await tester.binding.setSurfaceSize(const Size(816, 500));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);
    await mouse.moveTo(
      tester.getCenter(find.byKey(const ValueKey('quick-paste-clip-0'))),
    );
    await tester.pump(const Duration(milliseconds: 1000));
    final scroll = tester
        .widget<ScrollView>(
          find.byKey(const ValueKey('quick-paste-history-scroll')),
        )
        .controller!;
    scroll.jumpTo(scroll.position.maxScrollExtent);
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.byKey(const ValueKey('quick-paste-clip-0')), findsNothing);
    expect(controller.inspectorOpen, isFalse);
    expect(host.inspectorChanges, isEmpty);
    await mouse.moveTo(Offset.zero);
    await tester.pump(const Duration(seconds: 2));
    expect(host.inspectorChanges, isEmpty);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'manual close suppresses the hovered clip until selection changes',
    (tester) async {
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
      final recent = repository.clips.last;
      controller.hoverClip(recent, hovered: true);
      await tester.pump(const Duration(milliseconds: 1500));
      expect(controller.inspectorOpen, isTrue);
      await controller.toggleInspector();
      controller.hoverClip(recent, hovered: false);
      controller.hoverClip(recent, hovered: true);
      await tester.pump(const Duration(seconds: 2));
      expect(controller.inspectorOpen, isFalse);
      expect(host.inspectorChanges, [true, false]);
      controller.hoverClip(repository.clips.first, hovered: true);
      await tester.pump(const Duration(milliseconds: 1500));
      expect(controller.inspectorOpen, isTrue);
      expect(controller.history.selectedClip?.id, 'pinned');
      expect(host.inspectorChanges, [true, false, true]);
    },
  );

  for (final action in [
    'search',
    'keyboard',
    'close',
    'reopen',
    'paste',
    'main',
    'settings',
    'quit',
    'shutdown',
    'delete',
  ]) {
    testWidgets('$action cancels a pending hover inspector', (tester) async {
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
      final recent = repository.clips.last;
      controller.hoverClip(recent, hovered: true);
      await tester.pump(const Duration(milliseconds: 1000));
      switch (action) {
        case 'search':
          await controller.search('new');
        case 'keyboard':
          await controller.moveFocus(1);
        case 'close':
          await controller.close();
        case 'reopen':
          await controller.opened(2);
        case 'paste':
          await controller.activate(recent);
        case 'main':
          await controller.openMainWindow();
        case 'settings':
          await controller.openSettings();
        case 'quit':
          await controller.quit();
        case 'shutdown':
          await controller.shutdown();
        case 'delete':
          await controller.history.deleteClip(recent.id);
        default:
          throw StateError('Unknown cancellation action: $action');
      }
      await tester.pump(const Duration(seconds: 2));
      expect(host.inspectorChanges, isEmpty);
    });
  }

  testWidgets('manual close wins over a pending automatic native open', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost()..inspectorGate = Completer<void>();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    controller.hoverClip(repository.clips.last, hovered: true);
    await tester.pump(const Duration(milliseconds: 1500));
    expect(host.inspectorChanges, [true]);
    final close = controller.toggleInspector();
    host.inspectorGate!.complete();
    await tester.pump();
    await close;
    expect(controller.inspectorOpen, isFalse);
    expect(host.inspectorChanges, [true, false]);
    expect(host.inspectorPresentationIds, [1, 1]);
    await tester.pump(const Duration(seconds: 2));
    expect(host.inspectorChanges, [true, false]);
  });

  testWidgets('hovering another clip can reopen after a pending manual close', (
    tester,
  ) async {
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
    controller.hoverClip(repository.clips.last, hovered: true);
    await tester.pump(const Duration(milliseconds: 1500));
    host.inspectorGate = Completer<void>();
    final close = controller.toggleInspector();
    await tester.pump();
    controller.hoverClip(repository.clips.first, hovered: true);
    await tester.pump(const Duration(milliseconds: 1500));
    expect(host.inspectorChanges, [true, false]);
    host.inspectorGate!.complete();
    await tester.pump();
    await close;
    expect(host.inspectorChanges, [true, false, true]);
    expect(controller.inspectorOpen, isTrue);
    expect(controller.history.selectedClip?.id, 'pinned');
  });

  testWidgets('a reopened popup ignores completion of an old hover open', (
    tester,
  ) async {
    final repository = _Repository();
    final host = _ContextHost()..inspectorGate = Completer<void>();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    controller.hoverClip(repository.clips.last, hovered: true);
    await tester.pump(const Duration(milliseconds: 1500));
    await controller.opened(2);
    host.inspectorGate!.complete();
    await tester.pump();
    expect(controller.inspectorOpen, isFalse);
    expect(repository.getCalls, 0);
    expect(host.inspectorPresentationIds, [1]);
  });

  testWidgets('slow inspector content never blocks manual closing', (
    tester,
  ) async {
    final repository = _Repository()..selectGate = Completer<HistoryClip>();
    final host = _ContextHost();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.opened(1);
    controller.hoverClip(repository.clips.last, hovered: true);
    await tester.pump(const Duration(milliseconds: 1500));
    expect(controller.inspectorOpen, isTrue);
    await controller.toggleInspector();
    expect(controller.inspectorOpen, isFalse);
    expect(host.inspectorChanges, [true, false]);
    repository.selectGate!.complete(repository.clips.last);
    await tester.pump();
    expect(controller.inspectorOpen, isFalse);
  });

  for (final failure in [
    PlatformException(code: 'window_unavailable'),
    MissingPluginException(),
  ]) {
    testWidgets('hover tolerates an unavailable native inspector: $failure', (
      tester,
    ) async {
      final repository = _Repository();
      final host = _ContextHost()..inspectorFailure = failure;
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.opened(1);
      controller.hoverClip(repository.clips.last, hovered: true);
      await tester.pump(const Duration(milliseconds: 1500));
      expect(controller.inspectorOpen, isFalse);
      expect(repository.getCalls, 0);
      expect(tester.takeException(), isNull);
    });
  }

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

  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.linux,
  ]) {
    test(
      'saved editing and search clip accelerators migrate on $platform',
      () async {
        debugDefaultTargetPlatformOverride = platform;
        addTearDown(() => debugDefaultTargetPlatformOverride = null);
        final repository = _Repository();
        final saved = {
          'pinned': 'c',
          'find': 'f',
          'refresh': 'r',
          'cut': 'x',
          'redo': 'y',
          'valid': 'b',
        };
        for (final id in saved.keys.where((id) => id != 'pinned')) {
          repository.clips.add(
            HistoryClip(
              id: id,
              contentType: 'text/plain',
              preview: id,
              createdAt: DateTime.utc(2026),
              pinned: true,
            ),
          );
        }
        final store = MemoryQuickPastePreferencesStore();
        await store.writePinnedShortcuts(saved);
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: store,
          host: _ContextHost(),
        );
        await controller.initialize();
        final migrated = await store.readPinnedShortcuts();
        expect(migrated['valid'], 'b');
        expect(migrated.values.toSet().length, saved.length);
        for (final reserved in ['c', 'f', 'r', 'x', 'y']) {
          expect(migrated.values, isNot(contains(reserved)));
        }
        controller.dispose();
        final restarted = QuickPasteController(
          repository: repository,
          preferencesStore: store,
          host: _ContextHost(),
        );
        addTearDown(restarted.dispose);
        await restarted.initialize();
        expect(await store.readPinnedShortcuts(), migrated);
      },
    );
  }

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

  for (final platform in [TargetPlatform.macOS, TargetPlatform.windows]) {
    testWidgets('inspector actions work and retain the popup on $platform', (
      tester,
    ) async {
      final repository = _Repository();
      final host = _ContextHost();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
      );
      await tester.binding.setSurfaceSize(const Size(816, 600));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      await controller.opened(1, inspectorVisible: true);
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const ValueKey('history-detail-secret')));
      await tester.pump();
      expect(controller.history.selectedClip!.secret, isTrue);
      await tester.tap(find.byKey(const ValueKey('history-detail-secret')));
      await tester.pumpAndSettle();
      expect(controller.history.selectedClip!.secret, isFalse);

      await tester.tap(find.byKey(const ValueKey('history-detail-copy')));
      await tester.pumpAndSettle();
      expect(repository.copied, ['recent']);
      await tester.tap(find.byKey(const ValueKey('history-copy-options')));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
      await tester.tap(find.text('Copy plain text'));
      await tester.pumpAndSettle();
      expect(repository.plainCopied, ['recent']);
      expect(host.closeCalls, 0);
      expect(host.pasteCalls, 0);

      controller.focus(controller.history.selectedClip!);
      final pin = find.byKey(const ValueKey('history-pin-recent'));
      await tester.tap(pin);
      await tester.pumpAndSettle();
      expect(controller.history.selectedClip!.pinned, isTrue);
      expect(controller.focusedClip!.pinned, isTrue);
      expect(controller.shortcutFor(controller.focusedClip!), isNotNull);
      expect(
        controller.shortcutFor(controller.focusedClip!),
        isNot(LogicalKeyboardKey.digit1),
      );
      await tester.tap(pin);
      await tester.pumpAndSettle();
      expect(controller.history.selectedClip!.pinned, isFalse);
      expect(
        controller.shortcutFor(controller.focusedClip!),
        LogicalKeyboardKey.digit1,
      );

      await tester.tap(find.byKey(const ValueKey('history-detail-delete')));
      await tester.pumpAndSettle();
      expect(find.text('Delete this clip?'), findsOneWidget);
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      expect(repository.clips.length, 2);
      await tester.tap(find.byKey(const ValueKey('history-detail-delete')));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(Button, 'Delete').last);
      await tester.pumpAndSettle();
      expect(repository.clips.map((clip) => clip.id), ['pinned']);
      expect(controller.focusedClip?.id, 'pinned');
      expect(controller.history.selectedClip?.id, 'pinned');
      expect(host.closeCalls, 0);
      expect(tester.takeException(), isNull);
      await tester.pump(const Duration(seconds: 6));
      await tester.pumpAndSettle();
      await tester.pumpWidget(const SizedBox.shrink());
    }, variant: TargetPlatformVariant({platform}));
  }

  testWidgets('inspector download saves, cancels, and reports failure', (
    tester,
  ) async {
    final repository = _Repository();
    repository.clips
      ..clear()
      ..add(
        HistoryClip(
          id: 'download',
          contentType: 'file',
          preview: 'report.pdf',
          createdAt: DateTime.utc(2026),
          pinned: false,
          file: const HistoryFileDetails(
            name: 'report.pdf',
            mimeType: 'application/pdf',
          ),
        ),
      );
    final downloader = _Downloader();
    final host = _ContextHost();
    final controller = QuickPasteController(
      repository: repository,
      preferencesStore: MemoryQuickPastePreferencesStore(),
      host: host,
      fileDownloader: downloader,
    );
    await tester.binding.setSurfaceSize(const Size(816, 600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(QuickPasteApp(controller: controller));
    await tester.pumpAndSettle();
    await controller.opened(1, inspectorVisible: true);
    await tester.pumpAndSettle();
    final download = find.byKey(const ValueKey('history-detail-download'));
    await tester.tap(download);
    await tester.pumpAndSettle();
    expect(repository.saved, [('download', '/tmp/report.pdf')]);
    expect(downloader.presented, ['/tmp/report.pdf']);
    repository.saveGate = Completer<void>();
    await tester.tap(download);
    await tester.pump();
    expect(
      find.descendant(of: download, matching: find.text('0%')),
      findsOneWidget,
    );
    final progressButtonSize = tester.getSize(download);
    expect(progressButtonSize.width, progressButtonSize.height);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(download));
    await tester.pump();
    await tester.pump();
    expect(
      find.descendant(of: download, matching: find.byIcon(LucideIcons.x)),
      findsOneWidget,
    );
    final cancelButtonSize = tester.getSize(download);
    expect(cancelButtonSize.width, cancelButtonSize.height);
    await tester.tap(download);
    await tester.pump();
    expect(controller.history.isDownloadCancelling('download'), isTrue);
    repository.saveGate!.complete();
    repository.saveGate = null;
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pumpAndSettle();
    expect(downloader.presented, ['/tmp/report.pdf']);
    expect(controller.history.isDownloadPending('download'), isFalse);
    await mouse.removePointer();
    downloader.destination = null;
    await tester.tap(download);
    await tester.pumpAndSettle();
    expect(repository.saved.length, 2);
    downloader.destination = '/tmp/report.pdf';
    repository.failSave = true;
    expect(tester.widget<Button>(download).onPressed, isNotNull);
    await tester.tap(download);
    await tester.pumpAndSettle();
    expect(
      controller.history.errorMessage,
      'This file could not be downloaded.',
    );
    expect(controller.history.downloadFailedFor('download'), isTrue);
    expect(
      find.descendant(
        of: download,
        matching: find.byIcon(LucideIcons.rotateCcw),
      ),
      findsOneWidget,
    );
    expect(tester.widget<Button>(download).onPressed, isNotNull);
    expect(host.closeCalls, 0);
    expect(tester.takeException(), isNull);
    await tester.pump(const Duration(seconds: 6));
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'Quick Paste image OCR recognizes and copies through the shared dialog',
    (tester) async {
      final repository = _Repository();
      repository.clips
        ..clear()
        ..add(
          HistoryClip(
            id: 'image',
            contentType: 'image/png',
            preview: '[Image]',
            createdAt: DateTime.utc(2026),
            pinned: false,
          ),
        );
      final moduleRepository = _OcrModules();
      moduleRepository.pending = Completer<ModuleResult>();
      final modules = ModulesController(
        repository: moduleRepository,
        marketplace: MemoryModuleMarketplace(),
      );
      await modules.initialize();
      addTearDown(modules.dispose);
      final input = _OcrInput();
      final clipboard = _OcrClipboard();
      final host = _ContextHost();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
        ocr: HistoryOcrController(
          modules: modules,
          imageInput: input,
          clipboard: clipboard,
        ),
      );
      await tester.binding.setSurfaceSize(const Size(816, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(QuickPasteApp(controller: controller));
      await tester.pumpAndSettle();
      await controller.opened(1, inspectorVisible: true);
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('history-ocr')));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));
      final loadingBounds = tester.getRect(find.byType(AlertDialog));
      final loadingCopy = tester.getRect(
        find.byKey(const ValueKey('history-ocr-copy')),
      );
      moduleRepository.pending!.complete(
        const ModuleResult('Recognized fixture text'),
      );
      await tester.pumpAndSettle();
      expect(tester.getRect(find.byType(AlertDialog)), loadingBounds);
      expect(
        tester.getRect(find.byKey(const ValueKey('history-ocr-copy'))),
        loadingCopy,
      );
      expect(find.text('Recognized fixture text'), findsOneWidget);
      expect(moduleRepository.lastArguments, {
        'image_path': '/tmp/ocr-fixture.png',
      });
      expect(input.cleaned, isTrue);
      await tester.tap(find.byKey(const ValueKey('history-ocr-copy')));
      await tester.pumpAndSettle();
      expect(clipboard.text, 'Recognized fixture text');
      expect(host.closeCalls, 0);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      await modules.setEnabled('copypaste.ocr', false);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('history-ocr')), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pump(const Duration(seconds: 6));
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  test(
    'derived rows and shortcuts are immutable and reused until rows change',
    () async {
      final repository = _Repository();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: _ContextHost(),
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      final items = controller.items;
      final shortcuts = controller.shortcuts;
      expect(identical(controller.items, items), isTrue);
      expect(identical(controller.shortcuts, shortcuts), isTrue);
      expect(() => items.clear(), throwsUnsupportedError);
      expect(() => shortcuts.clear(), throwsUnsupportedError);
      await controller.history.select('recent');
      expect(identical(controller.items, items), isTrue);
      expect(identical(controller.shortcuts, shortcuts), isTrue);
      repository.clips.removeWhere((clip) => clip.id == 'recent');
      await controller.search('changed');
      expect(controller.items.map((clip) => clip.id), ['pinned']);
      expect(
        controller.shortcuts.containsKey(LogicalKeyboardKey.digit1),
        isFalse,
      );
    },
  );

  test(
    'unrelated history notifications do not persist pinned shortcuts',
    () async {
      final repository = _Repository();
      final store = _CountingPreferences();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: store,
        host: _ContextHost(),
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await Future<void>.delayed(Duration.zero);
      final writes = store.writes;
      await controller.history.select('recent');
      await controller.history.select('pinned');
      await Future<void>.delayed(Duration.zero);
      expect(store.writes, writes);
    },
  );

  test(
    'pin changes during shortcut persistence are reconciled in order',
    () async {
      final repository = _Repository();
      final store = _CountingPreferences();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: store,
        host: _ContextHost(),
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await Future<void>.delayed(Duration.zero);
      store.gate = Completer<void>();
      store.started = Completer<void>();
      final second = repository.clips.last.copyWith(pinned: true);
      repository.clips[1] = second;
      await controller.search('first');
      await store.started!.future;
      repository.clips[1] = second.copyWith(pinned: false);
      await controller.search('second');
      store.gate!.complete();
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);
      final saved = await store.readPinnedShortcuts();
      expect(saved.containsKey(second.id), isFalse);
      expect(controller.shortcuts[LogicalKeyboardKey.digit1]?.id, second.id);
    },
  );

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
        LogicalKeyboardKey.keyC,
        LogicalKeyboardKey.keyF,
        LogicalKeyboardKey.keyR,
        LogicalKeyboardKey.keyX,
        LogicalKeyboardKey.keyY,
      ]) {
        expect(keys, isNot(contains(reserved)));
      }
    },
  );

  for (final platform in [TargetPlatform.macOS, TargetPlatform.windows]) {
    test(
      'pinned shortcuts respect native window commands on $platform',
      () async {
        debugDefaultTargetPlatformOverride = platform;
        addTearDown(() => debugDefaultTargetPlatformOverride = null);
        final store = MemoryQuickPastePreferencesStore();
        await store.writePinnedShortcuts({
          'pinned': 'h',
          'second-pin': 'm',
          'unchanged': 'b',
        });
        final repository = _Repository();
        for (final id in ['second-pin', 'unchanged']) {
          repository.clips.add(
            HistoryClip(
              id: id,
              contentType: 'text/plain',
              preview: id,
              createdAt: DateTime.utc(2026),
              pinned: true,
            ),
          );
        }
        final controller = QuickPasteController(
          repository: repository,
          preferencesStore: store,
          host: _ContextHost()..permissionGranted = true,
        );
        addTearDown(controller.dispose);
        await controller.initialize();
        final saved = await store.readPinnedShortcuts();
        expect(saved['unchanged'], 'b');
        if (platform == TargetPlatform.macOS) {
          expect(saved.values, isNot(contains('h')));
          expect(saved.values, isNot(contains('m')));
          expect(
            controller.shortcuts,
            isNot(contains(LogicalKeyboardKey.keyH)),
          );
          expect(
            controller.shortcuts,
            isNot(contains(LogicalKeyboardKey.keyM)),
          );
        } else {
          expect(saved['pinned'], 'h');
          expect(saved['second-pin'], 'm');
        }
        expect(saved.values.toSet().length, 3);
        controller.dispose();
        final restarted = QuickPasteController(
          repository: repository,
          preferencesStore: store,
          host: _ContextHost()..permissionGranted = true,
        );
        addTearDown(restarted.dispose);
        await restarted.initialize();
        expect(await store.readPinnedShortcuts(), saved);
      },
    );
  }

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
      expect(host.pasteCalls, 1);
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
        expect(host.pasteCalls, 1);
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

  test(
    'retained shutdown completes when deferred action cleanup throws',
    () async {
      final repository = _Repository();
      final host = _ContextHost()..permissionGranted = true;
      final operations = _ThrowingDisposeOperations();
      final controller = QuickPasteController(
        repository: repository,
        preferencesStore: MemoryQuickPastePreferencesStore(),
        host: host,
        ownsHostLifecycle: false,
        fileOperations: operations,
      );
      await controller.initialize();
      await controller.opened(1);
      repository.copyGate = Completer<void>();
      final action = controller.activate(controller.history.items.last);
      await repository.copyStarted.future;
      final shutdown = controller.shutdown();
      final failedAction = expectLater(action, throwsStateError);
      repository.copyGate!.complete();
      await failedAction;
      await shutdown;
      expect(host.pasteIds, isEmpty);
      expect(host.closeIds, isEmpty);
      // Finish the deliberately failed cleanup without leaving test watchers.
      controller.history.dispose();
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
  @override
  Future<void> importFile(HistoryImportFile file) async {}

  HistorySourceAppIcon? sourceIcon;
  int sourceIconCalls = 0;
  int queryCalls = 0;
  int facetsCalls = 0;
  final clips = <HistoryClip>[
    HistoryClip(
      id: 'pinned',
      contentType: 'text/plain',
      preview: 'Pinned clip',
      createdAt: DateTime.utc(2026),
      pinned: true,
      sourceApp: 'Editor',
      sourceAppIconId: 'app:editor',
    ),
    HistoryClip(
      id: 'recent',
      contentType: 'text/plain',
      preview: 'Recent clip',
      createdAt: DateTime.utc(2026),
      pinned: false,
      sourceApp: 'Editor',
      sourceAppIconId: 'app:editor',
    ),
  ];
  final copied = <String>[];
  final plainCopied = <String>[];
  final saved = <(String, String)>[];
  bool failSave = false;
  bool failCopy = false, failPin = false, failDelete = false;
  List<int>? savedBytes;
  Completer<void>? saveGate;
  final saveStarted = Completer<void>();
  final imagePreviewEdges = <int?>[];
  final imagePreviewIds = <String>[];
  final imagePreviewBounds = <(int, int)>[];
  Completer<void>? imageLoadGate;
  int deleteAllCalls = 0;
  int getCalls = 0;
  Completer<void>? copyGate;
  Completer<void> copyStarted = Completer<void>();
  Completer<HistoryClip>? selectGate;
  Completer<void> selectStarted = Completer<void>();

  @override
  Future<void> copy(String id) async {
    if (failCopy) throw StateError('Copy failed');
    copied.add(id);
    if (!copyStarted.isCompleted) copyStarted.complete();
    await copyGate?.future;
  }

  @override
  Future<void> copyText(String id, String text) async {}

  @override
  Future<void> copyPlainText(String id) async {
    plainCopied.add(id);
    await copy(id);
  }

  @override
  Future<void> saveFile(String id, String destinationPath) async {
    if (failSave) throw StateError('Save failed');
    saved.add((id, destinationPath));
    if (!saveStarted.isCompleted) saveStarted.complete();
    final gate = saveGate;
    final bytes = savedBytes;
    if (bytes != null) {
      final file = File(destinationPath);
      await file.writeAsBytes(bytes.take(bytes.length ~/ 2).toList());
      await gate?.future;
      await file.writeAsBytes(
        bytes.skip(bytes.length ~/ 2).toList(),
        mode: FileMode.append,
      );
    } else {
      await gate?.future;
    }
  }

  @override
  Future<void> delete(String id) async {
    if (failDelete) throw StateError('Delete failed');
    clips.removeWhere((clip) => clip.id == id);
  }

  @override
  Future<void> deleteAll() async {
    deleteAllCalls += 1;
    clips.removeWhere((clip) => !clip.pinned);
  }

  @override
  Future<HistoryFacets> facets() async {
    facetsCalls += 1;
    return const HistoryFacets();
  }

  @override
  Future<HistoryClip> get(String id) async {
    getCalls += 1;
    if (!selectStarted.isCompleted) selectStarted.complete();
    return await selectGate?.future ??
        clips.singleWhere((clip) => clip.id == id);
  }

  @override
  Future<HistoryImagePreview?> imagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) async {
    imagePreviewEdges.add(maxEdge);
    imagePreviewIds.add(id);
    if (bounds != null) imagePreviewBounds.add((bounds.width, bounds.height));
    if (id != 'image') return null;
    await imageLoadGate?.future;
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
  }) async {
    queryCalls += 1;
    return HistoryClipPage(items: List.of(clips));
  }

  @override
  Future<void> reorderPinned(List<String> ids) async {}

  @override
  Future<HistoryClip> setSecret(String id, bool secret) async {
    final index = clips.indexWhere((clip) => clip.id == id);
    return clips[index] = clips[index].copyWith(secret: secret);
  }

  @override
  Future<void> setPinned(String id, bool pinned) async {
    if (failPin) throw StateError('Pin failed');
  }

  @override
  Future<HistorySourceAppIcon?> sourceAppIcon(String id) async {
    sourceIconCalls += 1;
    return sourceIcon == null
        ? null
        : HistorySourceAppIcon(Uint8List.fromList(sourceIcon!.bytes));
  }

  @override
  Stream<HistoryRuntimeEvent> watch() =>
      Stream<HistoryRuntimeEvent>.multi((_) {});
}

void _populateHistory(
  _Repository repository,
  int count, {
  int pinnedCount = 0,
  bool images = false,
}) {
  repository.clips
    ..clear()
    ..addAll([
      for (var index = 0; index < count; index++)
        HistoryClip(
          id: 'clip-$index',
          contentType: images ? 'image/png' : 'text/plain',
          preview: 'Clip $index',
          createdAt: DateTime.utc(2026),
          pinned: index < pinnedCount,
        ),
    ]);
}

class _PagedRepository extends _Repository {
  final requests = <({int limit, String? cursor, String search})>[];
  bool failNextPage = false;
  Completer<void>? pageGate;
  final pageStarted = Completer<void>();

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    requests.add((limit: limit, cursor: cursor, search: query.search));
    if (cursor != null) {
      if (!pageStarted.isCompleted) pageStarted.complete();
      await pageGate?.future;
      if (failNextPage) {
        failNextPage = false;
        throw StateError('Page unavailable');
      }
    }
    final matches = clips.where((clip) => clip.preview.contains(query.search));
    final ordered = [
      ...matches.where((clip) => clip.pinned),
      ...matches.where((clip) => !clip.pinned),
    ];
    final offset = cursor == null ? 0 : int.parse(cursor);
    final page = ordered.skip(offset).take(limit).toList();
    final next = offset + page.length;
    return HistoryClipPage(
      items: page,
      nextCursor: next < ordered.length ? '$next' : null,
    );
  }
}

class _ThrowingDisposeOperations extends FileOperationsController {
  _ThrowingDisposeOperations() : super(_RetainedOperations());

  bool _throwOnce = true;

  @override
  void dispose() {
    if (_throwOnce) {
      _throwOnce = false;
      throw StateError('synthetic cleanup failure');
    }
    super.dispose();
  }
}

class _RetainedRepository extends _Repository {
  int activeWatches = 0;
  Completer<void>? queryGate;
  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    await queryGate?.future;
    return super.query(query: query, limit: limit, cursor: cursor);
  }

  @override
  Stream<HistoryRuntimeEvent> watch() => Stream.multi((sink) {
    activeWatches++;
    sink.onCancel = () {
      activeWatches--;
    };
  });
}

class _RetainedOperations implements FileOperationsRepository {
  int activeWatches = 0;
  @override
  Stream<FileOperationsChange> watch() => Stream.multi((sink) {
    activeWatches++;
    sink.onCancel = () {
      activeWatches--;
    };
  });
  @override
  Future<List<FileOperation>> load({
    bool clear = false,
    String? before,
    String? id,
  }) async => [];
  @override
  Future<void> retry(FileOperation operation) async {}
  @override
  Future<void> cancel(String id) async {}
  @override
  Future<void> openFile(String path) async {}
}

class _ContextHost implements RetainedQuickPasteContextHost {
  Future<void> Function(int)? hiddenHandler;
  @override
  void setHiddenHandler(Future<void> Function(int)? handler) =>
      hiddenHandler = handler;
  @override
  void setShutdownHandler(Future<void> Function()? handler) {
    shutdownHandler = handler;
  }

  @override
  Future<void> signalReady() async {
    readyCalls += 1;
    if (openWhenReady) {
      await openedHandler?.call(1, inspectorVisible: initialInspectorVisible);
    }
  }

  Future<void> Function()? shutdownHandler;
  int readyCalls = 0;
  bool openWhenReady = false;
  bool initialInspectorVisible = false;
  final inspectorChanges = <bool>[];
  final inspectorPresentationIds = <int>[];
  final inspectorRemembered = <bool>[];
  Completer<void>? inspectorGate;
  Object? inspectorFailure;

  @override
  Future<void> setInspectorVisible({
    required int presentationId,
    required bool visible,
    bool remember = true,
  }) async {
    inspectorChanges.add(visible);
    inspectorRemembered.add(remember);
    inspectorPresentationIds.add(presentationId);
    final failure = inspectorFailure;
    if (failure != null) throw failure;
    await inspectorGate?.future;
  }

  bool permissionGranted = false;
  int permissionRequests = 0;
  int pasteCalls = 0;
  int closeCalls = 0;
  int quitCalls = 0;
  QuickPasteOpenedHandler? openedHandler;
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
  Future<void> quit() async {
    quitCalls += 1;
  }

  @override
  Future<bool> requestAccessibility() async {
    permissionRequests += 1;
    return permissionGranted;
  }

  @override
  void setOpenedHandler(QuickPasteOpenedHandler? handler) {
    openedHandler = handler;
  }
}

class _Downloader implements HistoryFileDownloader {
  String? destination = '/tmp/report.pdf';
  final presented = <String>[];
  @override
  Future<String?> chooseDestination(HistoryFileDetails file) async =>
      destination;
  @override
  Future<void> presentSavedFile(String path, HistoryFileDetails file) async {
    presented.add(path);
  }
}

class _OcrModules extends MemoryModulesRepository {
  Completer<ModuleResult>? pending;
  _OcrModules() {
    modules = [
      const InstalledModule(
        id: 'copypaste.ocr',
        title: 'OCR',
        description: '',
        version: '1.0.0',
        enabled: true,
        sizeBytes: 1,
        commands: [
          ModuleCommand(
            id: 'recognize-image',
            title: 'Recognize image text',
            description: '',
            arguments: [],
          ),
        ],
        preferenceFields: [],
        preferences: {},
      ),
    ];
  }
  @override
  Future<ModuleResult> invoke(
    String id,
    String command,
    Map<String, Object> arguments,
  ) async {
    lastArguments = arguments;
    return pending == null
        ? const ModuleResult('Recognized fixture text')
        : await pending!.future;
  }
}

class _OcrInput implements HistoryImageInput {
  bool cleaned = false;
  @override
  Future<SelectedModuleInput> prepare(HistoryClip clip) async =>
      SelectedModuleInput(
        path: '/tmp/ocr-fixture.png',
        name: 'ocr-fixture.png',
        dispose: () async {
          cleaned = true;
        },
      );
}

class _OcrClipboard implements ClipboardWriter {
  String? text;
  @override
  Future<void> writeText(String text) async {
    this.text = text;
  }
}

class _FrameHost extends _ContextHost implements QuickPasteFrameHost {
  final presentedIds = <int>[];

  @override
  Future<bool> presentFrame({required int presentationId}) async {
    presentedIds.add(presentationId);
    return true;
  }
}

class _CountingPreferences extends MemoryQuickPastePreferencesStore {
  int writes = 0;
  Completer<void>? gate;
  Completer<void>? started;

  @override
  Future<void> writePinnedShortcuts(Map<String, String> shortcuts) async {
    writes++;
    if (started case final value? when !value.isCompleted) value.complete();
    await gate?.future;
    await super.writePinnedShortcuts(shortcuts);
  }
}

class _FailedFileTransfer extends HistoryFileTransfer {
  _FailedFileTransfer(super.repository);
  @override
  Future<String> prepare(
    HistoryClip clip, {
    required void Function(double) onProgress,
  }) async => throw const HistoryFileOperationException(
    'The source device is offline. Bring it online and retry.',
  );
}

class _WaitingFileTransfer extends HistoryFileTransfer {
  _WaitingFileTransfer(super.repository);
  final finish = Completer<String>();
  @override
  Future<String> prepare(
    HistoryClip clip, {
    required void Function(double) onProgress,
  }) {
    onProgress(0.25);
    return finish.future;
  }
}
