import 'dart:async';
import 'dart:io';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:copypaste_flutter/platform/files/history_file_drop_target.dart';
import 'package:copypaste_flutter/platform/files/history_file_picker.dart';
import 'package:file_selector/file_selector.dart';
import 'package:desktop_drop/desktop_drop.dart' as native;
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test(
    'desktop import preserves the leaf filename for either Windows separator',
    () {
      final input = SystemHistoryFilePicker.fromDesktopFile(
        XFile('/drop/a.pdf'),
      );
      expect(input.name, 'a.pdf');
      expect(input.mimeType, 'application/pdf');
      expect(input.sourceReference, '/drop/a.pdf');
      if (Platform.isWindows) {
        final windows = SystemHistoryFilePicker.fromDesktopFile(
          XFile(r'C:\drop\a.pdf'),
        );
        expect(windows.name, 'a.pdf');
        expect(windows.mimeType, 'application/pdf');
      }
    },
  );
  test(
    'imports every selection sequentially with no selection-count cap',
    () async {
      final repository = _Repository();
      final files = List.generate(1501, (index) => _Input('$index.pdf'));
      final controller = HistoryController(
        repository,
        filePicker: _Picker(files),
      );
      await controller.initialize();
      final queries = repository.queries;
      await controller.chooseFiles();
      expect(repository.imported, files.map((file) => file.name));
      expect(repository.maximumActive, 1);
      expect(files.every((file) => file.disposed), isTrue);
      expect(repository.queries, queries + 1);
      expect(controller.isImportingFiles, isFalse);
      expect(controller.errorMessage, isNull);
      controller.dispose();
      await repository.events.close();
    },
  );

  test('cancellation does not import, refresh or report an error', () async {
    final repository = _Repository();
    final controller = HistoryController(repository, filePicker: _Picker([]));
    await controller.initialize();
    final queries = repository.queries;
    await controller.chooseFiles();
    expect(repository.imported, isEmpty);
    expect(repository.queries, queries);
    expect(controller.errorMessage, isNull);
    controller.dispose();
    await repository.events.close();
  });

  test(
    'refreshes background captures even when the file picker is cancelled',
    () async {
      final repository = _Repository();
      final picker = _Picker([])
        ..pending = Completer<List<HistoryImportFile>>();
      final controller = HistoryController(repository, filePicker: picker);
      await controller.initialize();
      final queries = repository.queries;
      final selection = controller.chooseFiles();
      expect(controller.canSuspend, isFalse);
      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      picker.pending!.complete([]);
      await selection;
      expect(controller.canSuspend, isTrue);
      expect(repository.queries, queries + 1);
      expect(repository.imported, isEmpty);
      expect(controller.errorMessage, isNull);
      controller.dispose();
      await repository.events.close();
    },
  );

  test('continues after one failure and releases every selection', () async {
    final repository = _Repository()..failName = 'unreadable.pdf';
    final files = [_Input('a.pdf'), _Input('unreadable.pdf'), _Input('b.pdf')];
    final controller = HistoryController(repository);
    await controller.initialize();
    await controller.importFiles(files);
    expect(repository.imported, ['a.pdf', 'b.pdf']);
    expect(files.every((file) => file.disposed), isTrue);
    expect(controller.errorMessage, contains('1 of 3 files'));
    expect(controller.errorMessage, contains('unreadable.pdf'));
    controller.dispose();
    await repository.events.close();
  });

  for (final width in [320.0, 480.0, 1400.0]) {
    testWidgets(
      'icon import remains accessible at width $width',
      (tester) async {
        await tester.binding.setSurfaceSize(Size(width, 900));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final repository = _Repository();
        final picker = _Picker([_Input('a.pdf')]);
        final controller = HistoryController(repository, filePicker: picker);
        addTearDown(controller.dispose);
        addTearDown(repository.events.close);
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            darkTheme: AppTheme.dark,
            builder: AppTheme.builder,
            home: Scaffold(child: HistoryScreen(controller: controller)),
          ),
        );
        await tester.pumpAndSettle();
        final button = find.byKey(const ValueKey('history-import-files'));
        expect(button, findsOneWidget);
        expect(
          find.descendant(
            of: button,
            matching: find.byIcon(LucideIcons.filePlus),
          ),
          findsOneWidget,
        );
        expect(
          find.descendant(of: button, matching: find.byType(Text)),
          findsNothing,
        );
        final controls = find.byWidgetPredicate((widget) => widget is Select);
        final importPosition = tester.getCenter(button);
        for (final control in controls.evaluate()) {
          final position = tester.getCenter(find.byWidget(control.widget));
          expect(
            position.dy < importPosition.dy ||
                (position.dy == importPosition.dy &&
                    position.dx < importPosition.dx),
            isTrue,
          );
        }
        await tester.tap(button);
        await tester.pumpAndSettle();
        expect(picker.calls, 1);
        expect(repository.imported, ['a.pdf']);
        expect(tester.takeException(), isNull);
        if (width == 320) {
          final search = find.byKey(const ValueKey('history-search-toggle'));
          final searchPosition = tester.getCenter(search);
          for (final control in controls.evaluate()) {
            final position = tester.getCenter(find.byWidget(control.widget));
            expect(
              searchPosition.dy < position.dy ||
                  (searchPosition.dy == position.dy &&
                      searchPosition.dx < position.dx),
              isTrue,
            );
          }
          await tester.tap(search);
          await tester.pumpAndSettle();
          expect(button, findsNothing);
          expect(find.byType(TextField), findsOneWidget);
          await tester.tap(find.byIcon(LucideIcons.x));
          await tester.pump(const Duration(milliseconds: 250));
          await tester.pumpAndSettle();
          expect(button, findsOneWidget);
          expect(search, findsOneWidget);
          expect(tester.takeException(), isNull);
        }
      },
      variant: TargetPlatformVariant({
        TargetPlatform.macOS,
        TargetPlatform.windows,
        TargetPlatform.android,
      }),
    );
  }

  testWidgets('external drop imports files and excludes directories and text', (
    tester,
  ) async {
    final received = <List<HistoryImportFile>>[];
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: HistoryFileDropTarget(
          enabled: true,
          onFiles: received.add,
          onHover: (_) {},
          child: const SizedBox(width: 300, height: 300),
        ),
      ),
    );
    final target = tester.widget<native.DropTarget>(
      find.byType(native.DropTarget),
    );
    target.onDragDone!(
      native.DropDoneDetails(
        files: [
          native.DropItemFile('/a.pdf'),
          native.DropItemDirectory('/directory', []),
        ],
        globalPosition: Offset.zero,
        localPosition: Offset.zero,
      ),
    );
    expect(received.single.map((file) => file.name), ['a.pdf']);
    target.onDragDone!(
      native.DropDoneDetails(
        files: [],
        rawText: 'https://example.com',
        globalPosition: Offset.zero,
        localPosition: Offset.zero,
      ),
    );
    expect(received.length, 1);
  }, skip: !Platform.isMacOS && !Platform.isWindows);
}

class _Input extends HistoryImportFile {
  _Input(String name)
    : super(
        name: name,
        mimeType: 'application/pdf',
        sourceReference: 'content://documents/$name',
        prepare: (_) async => '/staged/$name',
        dispose: () async {},
      );
  bool disposed = false;
  @override
  Future<void> Function() get dispose => () async {
    disposed = true;
  };
}

class _Picker implements HistoryFilePicker {
  _Picker(this.files);
  final List<HistoryImportFile> files;
  int calls = 0;
  Completer<List<HistoryImportFile>>? pending;
  @override
  Future<List<HistoryImportFile>> chooseFiles() async {
    calls++;
    return pending == null ? files : await pending!.future;
  }
}

class _Repository implements HistoryRepository {
  final events = StreamController<HistoryRuntimeEvent>.broadcast(sync: true);
  final imported = <String>[];
  String? failName;
  int active = 0, maximumActive = 0, queries = 0;
  @override
  Future<void> importFile(HistoryImportFile file) async {
    active++;
    if (active > maximumActive) maximumActive = active;
    try {
      await file.prepare(4194304);
      if (file.name == failName) throw StateError('unreadable');
      imported.add(file.name);
      events.add(HistoryRuntimeEvent.itemsChanged);
    } finally {
      active--;
    }
  }

  @override
  Stream<HistoryRuntimeEvent> watch() => events.stream;
  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();
  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async {
    queries++;
    return const HistoryClipPage(items: []);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
