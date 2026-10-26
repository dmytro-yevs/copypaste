import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/transfers/file_operations_controller.dart';
import 'package:copypaste_flutter/features/transfers/file_operations_view.dart';

FileOperation operation(
  String id,
  String phase, {
  int written = 0,
  String? path,
  String? itemId,
  DateTime? startedAt,
}) => FileOperation(
  id: id,
  itemId: itemId ?? 'clip-$id',
  name: '$id.pdf',
  kind: FileOperationKind.receive,
  phase: phase,
  sourceDevice: 'Android phone',
  destinationDevice: 'MacBook Air',
  written: written,
  total: 1024 * 1024,
  startedAt: startedAt ?? DateTime(2026, 10, 21, 10),
  finishedAt: phase == 'running' ? null : DateTime(2026, 10, 21, 10, 1),
  localPath: path,
  error: phase == 'failed'
      ? 'The source device is unavailable. Bring it online and try again.'
      : null,
);

void main() {
  test(
    'independent screens share progress and reconnect without cancelling the worker',
    () async {
      final repository = _Repository()
        ..records = [operation('report', 'running')];
      final first = FileOperationsController(repository);
      final second = FileOperationsController(repository);
      await first.initialize();
      await second.initialize();
      final queries = repository.loads;
      repository.changes.add(
        const FileOperationsChange(
          id: 'report',
          written: 512 * 1024,
          total: 1024 * 1024,
          finished: false,
        ),
      );
      await Future<void>.delayed(Duration.zero);
      expect(first.forClip('clip-report')!.progress, 0.5);
      expect(second.forClip('clip-report')!.progress, 0.5);
      expect(
        repository.loads,
        queries,
        reason: 'Byte progress must not reload history',
      );
      first.dispose();
      expect(repository.cancelled, isEmpty);
      repository.records = [
        operation(
          'report',
          'completed',
          written: 1024 * 1024,
          path: '/saved/report.pdf',
        ),
      ];
      repository.changes.add(const FileOperationsChange(id: 'report'));
      await Future<void>.delayed(Duration.zero);
      expect(second.forClip('clip-report')!.localPath, '/saved/report.pdf');
      final reopened = FileOperationsController(repository);
      await reopened.initialize();
      expect(reopened.forClip('clip-report')!.phase, 'completed');
      await reopened.openFile(reopened.items.single);
      expect(repository.opened, ['/saved/report.pdf']);
      repository.records = [
        ...repository.records,
        operation('another', 'running'),
      ];
      await reopened.clearHistory();
      expect(reopened.items.single.id, 'another');
      expect(repository.cancelled, isEmpty);
      reopened.dispose();
      second.dispose();
      await repository.changes.close();
    },
  );

  test(
    'newest failure survives refresh, a second screen and reconnect',
    () async {
      final older = operation(
        'older',
        'completed',
        itemId: 'same-file',
        path: '/saved/report.pdf',
        startedAt: DateTime(2026, 10, 20),
      );
      final failed = operation(
        'newer',
        'failed',
        itemId: 'same-file',
        path: '/saved/retry.pdf',
        startedAt: DateTime(2026, 10, 21),
      );
      final repository = _Repository()..records = [older];
      final first = FileOperationsController(repository);
      await first.initialize();
      expect(first.forClip('same-file')!.phase, 'completed');
      repository.records = [failed, older];
      await first.refresh();
      expect(first.items.first.phase, 'failed');
      expect(first.forClip('same-file')!.id, 'newer');
      expect(first.forClip('same-file')!.error, contains('unavailable'));
      final second = FileOperationsController(repository);
      await second.initialize();
      expect(second.forClip('same-file')!.phase, 'failed');
      repository.changes.add(const FileOperationsChange());
      await Future<void>.delayed(Duration.zero);
      expect(first.forClip('same-file')!.phase, 'failed');
      expect(second.forClip('same-file')!.phase, 'failed');
      first.dispose();
      second.dispose();
      await repository.changes.close();
    },
  );

  test(
    'history removal preserves active records and downloaded-file paths',
    () async {
      final removable = operation(
        'complete-one',
        'completed',
        path: '/saved/one.pdf',
      );
      final retained = operation(
        'complete-two',
        'completed',
        path: '/saved/two.pdf',
      );
      final active = operation('active', 'running');
      final repository = _Repository()..records = [removable, retained, active];
      final controller = FileOperationsController(repository);
      await controller.initialize();

      await controller.removeHistory(active);
      expect(repository.clearIds, isEmpty);
      await controller.removeHistory(removable);

      expect(repository.clearIds, ['complete-one']);
      expect(controller.items.map((item) => item.id), [
        'active',
        'complete-two',
      ]);
      expect(
        repository.records
            .singleWhere((item) => item.id == 'complete-two')
            .localPath,
        '/saved/two.pdf',
      );
      expect(
        repository.records.singleWhere((item) => item.id == 'active').active,
        isTrue,
      );
      controller.dispose();
      await repository.changes.close();

      final failingRepository = _Repository()
        ..records = [removable]
        ..failClear = true;
      final failingController = FileOperationsController(failingRepository);
      await failingController.initialize();
      await failingController.removeHistory(removable);
      expect(failingController.forClip(removable.itemId)?.id, removable.id);
      failingController.dispose();
      await failingRepository.changes.close();
    },
  );

  test(
    'speed uses only fresh progress samples and is cleared at completion',
    () async {
      var now = Duration.zero;
      final repository = _Repository()
        ..records = [operation('report', 'running')];
      final controller = FileOperationsController(repository, now: () => now);
      await controller.initialize();

      repository.changes.add(
        const FileOperationsChange(
          id: 'report',
          written: 100,
          total: 1024 * 1024,
          finished: false,
        ),
      );
      await Future<void>.delayed(Duration.zero);
      expect(controller.speedFor('report'), isNull);

      now = const Duration(seconds: 1);
      repository.changes.add(
        const FileOperationsChange(
          id: 'report',
          written: 300,
          total: 1024 * 1024,
          finished: false,
        ),
      );
      await Future<void>.delayed(Duration.zero);
      expect(controller.speedFor('report'), 200);

      now = const Duration(seconds: 7);
      expect(controller.speedFor('report'), isNull);
      repository.records = [
        operation('report', 'completed', written: 1024 * 1024),
      ];
      repository.changes.add(const FileOperationsChange(id: 'report'));
      await Future<void>.delayed(Duration.zero);
      expect(controller.speedFor('report'), isNull);
      controller.dispose();
      await repository.changes.close();
    },
  );

  testWidgets(
    'manager shows bytes, devices, saved-file link and a fixed clear action',
    (tester) async {
      await tester.runAsync(() async {
        for (final font in [
          ('GeistSans', 'lib/fonts/Geist-Regular.otf'),
          ('LucideIcons', 'lib/icons/LucideIcons.ttf'),
        ]) {
          await (FontLoader(
                'packages/shadcn_flutter/${font.$1}',
              )..addFont(rootBundle.load('packages/shadcn_flutter/${font.$2}')))
              .load();
        }
      });
      await tester.binding.setSurfaceSize(const Size(1100, 820));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _Repository()
        ..records = [
          operation('video', 'running', written: 256 * 1024),
          operation(
            'invoice',
            'completed',
            written: 1024 * 1024,
            path: '/saved/invoice.pdf',
          ),
          operation('presentation', 'failed', path: '/saved/presentation.pdf'),
        ];
      final controller = FileOperationsController(repository);
      await controller.initialize();
      final boundary = GlobalKey();
      await tester.pumpWidget(
        RepaintBoundary(
          key: boundary,
          child: ShadcnApp(
            debugShowCheckedModeBanner: false,
            theme: AppTheme.dark.copyWith(
              typography: () => AppTheme.dark.typography.copyWith(
                sans: () => const TextStyle(
                  fontFamily: 'GeistSans',
                  package: 'shadcn_flutter',
                ),
              ),
            ),
            home: Scaffold(
              headers: [
                AppBar(
                  title: const Text('History'),
                  trailing: [
                    FileOperationsHeaderAction(controller: controller),
                  ],
                ),
              ],
              child: const SizedBox.expand(),
            ),
          ),
        ),
      );
      await tester.tap(find.byKey(const ValueKey('file-operations-open')));
      await tester.pump();
      await tester.pump(const Duration(seconds: 1));
      expect(find.text('Downloads and transfers'), findsOneWidget);
      expect(find.text('256.0 KB / 1.0 MB'), findsOneWidget);
      expect(find.text('Android phone'), findsNWidgets(3));
      expect(find.text('MacBook Air'), findsNWidgets(3));
      expect(find.text('Show file'), findsOneWidget);
      expect(find.textContaining('Bring it online'), findsOneWidget);
      expect(find.text('Clear completed history'), findsOneWidget);
      await tester.ensureVisible(find.text('Retry'));
      await tester.pump();
      await tester.tap(find.text('Retry'));
      await tester.pump();
      expect(repository.retried, ['presentation']);
      await tester.ensureVisible(find.text('Show file'));
      await tester.pump();
      await tester.tap(find.text('Show file'));
      expect(repository.opened, ['/saved/invoice.pdf']);
      await tester.pump();
      expect(tester.takeException(), isNull);
      final evidence = Platform.environment['COPYPASTE_EVIDENCE_DIR'];
      if (evidence != null) {
        // Capture the complete overlay tree, not just the page beneath the drawer.
        await tester.runAsync(() async {
          final pixels =
              await (boundary.currentContext!.findRenderObject()!
                      as RenderRepaintBoundary)
                  .toImage();
          final bytes = await pixels.toByteData(format: ui.ImageByteFormat.png);
          await File(
            '$evidence/file-transfer-manager.png',
          ).writeAsBytes(bytes!.buffer.asUint8List());
          pixels.dispose();
        });
      }
      await tester.pumpWidget(const SizedBox.shrink());
      controller.dispose();
      await repository.changes.close();
    },
  );
}

class _Repository implements FileOperationsRepository {
  final changes = StreamController<FileOperationsChange>.broadcast();
  List<FileOperation> records = [];
  final List<String> cancelled = [], opened = [], retried = [];
  final List<String?> clearIds = [];
  bool failClear = false;
  int loads = 0;
  @override
  Stream<FileOperationsChange> watch() => changes.stream;
  @override
  Future<List<FileOperation>> load({
    bool clear = false,
    String? before,
    String? id,
  }) async {
    loads++;
    if (clear) {
      clearIds.add(id);
      if (failClear) throw StateError('storage unavailable');
      records = id == null
          ? records.where((item) => item.active).toList()
          : records.where((item) => item.active || item.id != id).toList();
    }
    return id == null
        ? records
        : records.where((item) => item.id == id).toList();
  }

  @override
  Future<void> cancel(String id) async {
    cancelled.add(id);
  }

  @override
  Future<void> retry(FileOperation operation) async {
    retried.add(operation.id);
  }

  @override
  Future<void> openFile(String path) async {
    opened.add(path);
  }
}
