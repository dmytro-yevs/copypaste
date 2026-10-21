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
}) => FileOperation(
  id: id,
  itemId: 'clip-$id',
  name: '$id.pdf',
  kind: FileOperationKind.receive,
  phase: phase,
  sourceDevice: 'Android phone',
  destinationDevice: 'MacBook Air',
  written: written,
  total: 1024 * 1024,
  startedAt: DateTime(2026, 10, 21, 10),
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
      expect(find.text('Downloading · 256.0 KB / 1.0 MB'), findsOneWidget);
      expect(find.text('Android phone → MacBook Air'), findsNWidgets(3));
      expect(find.text('Show saved file'), findsOneWidget);
      expect(find.textContaining('Bring it online'), findsOneWidget);
      expect(find.text('Clear completed history'), findsOneWidget);
      await tester.ensureVisible(find.text('Retry'));
      await tester.pump();
      await tester.tap(find.text('Retry'));
      await tester.pump();
      expect(repository.retried, ['presentation']);
      await tester.ensureVisible(find.text('Show saved file'));
      await tester.pump();
      await tester.tap(find.text('Show saved file'));
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
    if (clear) records = records.where((item) => item.active).toList();
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
