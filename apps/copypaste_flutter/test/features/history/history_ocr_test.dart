import 'dart:async';
import 'dart:io';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_ocr_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_image_input.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/repository/temporary_history_image_input.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/modules_repository.dart';
import 'package:copypaste_flutter/platform/clipboard/clipboard_writer.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../modules/modules_test_support.dart';

final _image = HistoryClip(
  id: 'screenshot',
  contentType: 'image/png',
  preview: '[Image]',
  createdAt: DateTime.utc(2026),
  pinned: false,
);

InstalledModule _ocrModule({bool enabled = true, bool restart = false}) =>
    InstalledModule(
      id: 'copypaste.ocr',
      title: 'OCR',
      description: 'Recognize image text locally.',
      version: '0.1.0',
      enabled: enabled,
      restartRequired: restart,
      sizeBytes: 1,
      commands: const [
        ModuleCommand(
          id: 'recognize-image',
          title: 'Recognize image text',
          description: '',
          arguments: [],
        ),
      ],
      preferenceFields: const [],
      preferences: const {},
    );

Future<
  ({
    ModulesController modules,
    _OcrModulesRepository repository,
    HistoryOcrController ocr,
    _ImageInput input,
    _Clipboard clipboard,
  })
>
_fixture() async {
  final repository = _OcrModulesRepository()..modules = [_ocrModule()];
  final modules = ModulesController(
    repository: repository,
    marketplace: MemoryModuleMarketplace(),
  );
  await modules.initialize();
  final input = _ImageInput();
  final clipboard = _Clipboard();
  final ocr = HistoryOcrController(
    modules: modules,
    imageInput: input,
    clipboard: clipboard,
  );
  return (
    modules: modules,
    repository: repository,
    ocr: ocr,
    input: input,
    clipboard: clipboard,
  );
}

void main() {
  test(
    'availability follows module enable, remove, and restart state',
    () async {
      final f = await _fixture();
      addTearDown(f.modules.dispose);
      addTearDown(f.ocr.dispose);
      expect(f.ocr.canRun, isTrue);
      await f.modules.setEnabled('copypaste.ocr', false);
      expect(f.ocr.available, isFalse);
      await f.ocr.recognize(_image);
      expect(f.input.prepared, isEmpty);
      await f.modules.setEnabled('copypaste.ocr', true);
      expect(f.ocr.available, isTrue);
      await f.ocr.recognize(
        HistoryClip(
          id: 'text',
          contentType: 'text',
          preview: 'Text',
          createdAt: DateTime.utc(2026),
          pinned: false,
        ),
      );
      expect(f.input.prepared, isEmpty);
      f.repository.modules = [_ocrModule(restart: true)];
      await f.modules.initialize();
      expect(f.ocr.available, isFalse);
      await f.modules.remove('copypaste.ocr');
      expect(f.ocr.available, isFalse);
    },
  );

  test('runs once for the captured clip and copies only on request', () async {
    final f = await _fixture();
    addTearDown(f.modules.dispose);
    addTearDown(f.ocr.dispose);
    f.repository.pending = Completer<ModuleResult>();
    final run = f.ocr.recognize(_image);
    await Future<void>.delayed(Duration.zero);
    expect(f.ocr.busy, isTrue);
    expect(f.ocr.canRun, isFalse);
    await f.ocr.recognize(_image);
    expect(f.input.prepared, ['screenshot']);
    expect(f.repository.invocations, [
      ('copypaste.ocr', 'recognize-image', '/private/original.png'),
    ]);
    expect(f.input.cleaned, 0);
    f.repository.pending!.complete(
      const ModuleResult('Розпізнаний текст\nText'),
    );
    await run;
    expect(f.ocr.text, 'Розпізнаний текст\nText');
    expect(f.input.cleaned, 1);
    expect(f.clipboard.values, isEmpty);
    expect(await f.ocr.copyText(), isTrue);
    expect(f.clipboard.values, ['Розпізнаний текст\nText']);
  });

  test('failed invocation cleans input and supports a fresh retry', () async {
    final f = await _fixture();
    addTearDown(f.modules.dispose);
    addTearDown(f.ocr.dispose);
    f.repository.failure = const ModulesException('Model unavailable.');
    await f.ocr.recognize(_image);
    expect(f.ocr.errorMessage, 'Model unavailable.');
    expect(f.input.cleaned, 1);
    expect(await f.ocr.copyText(), isFalse);
    f.repository.failure = null;
    f.repository.result = '   ';
    await f.ocr.recognize(_image);
    expect(f.ocr.errorMessage, isNull);
    expect(f.input.cleaned, 2);
    expect(await f.ocr.copyText(), isFalse);
  });

  test('disposing during recognition still cleans input', () async {
    final f = await _fixture();
    addTearDown(f.modules.dispose);
    f.repository.pending = Completer<ModuleResult>();
    final run = f.ocr.recognize(_image);
    await Future<void>.delayed(Duration.zero);
    f.ocr.dispose();
    f.repository.pending!.complete(const ModuleResult('Ignored'));
    await run;
    expect(f.input.cleaned, 1);
    expect(f.ocr.text, isNull);
  });

  test(
    'temporary input exports original bytes and cleans failed exports',
    () async {
      final root = await Directory.systemTemp.createTemp('ocr-test-');
      addTearDown(() => root.delete(recursive: true));
      final history = _HistoryRepository();
      final input = TemporaryHistoryImageInput(
        repository: history,
        temporaryDirectory: () async => root,
      );
      final selected = await input.prepare(_image);
      expect(history.savedIds, ['screenshot']);
      expect(await File(selected.path).readAsBytes(), [1, 2, 3, 4]);
      await selected.dispose();
      expect(await root.list().toList(), isEmpty);
      history.exportFails = true;
      await expectLater(input.prepare(_image), throwsStateError);
      expect(await root.list().toList(), isEmpty);
    },
  );

  for (final (platform, size) in [
    (TargetPlatform.macOS, const Size(1400, 900)),
    (TargetPlatform.windows, const Size(1400, 900)),
    (TargetPlatform.android, const Size(390, 844)),
  ]) {
    testWidgets('inspector OCR result and Copy at $size', (tester) async {
      await tester.binding.setSurfaceSize(size);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final f = await _fixture();
      addTearDown(f.modules.dispose);
      f.repository.pending = Completer<ModuleResult>();
      final history = _HistoryRepository();
      final controller = HistoryController(history, ocr: f.ocr);
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          darkTheme: AppTheme.dark,
          builder: AppTheme.builder,
          home: Scaffold(child: HistoryScreen(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('history-clip-screenshot')));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('history-ocr')), findsOneWidget);
      final ocrButton = tester.widget<Button>(
        find.byKey(const ValueKey('history-ocr')),
      );
      expect(ocrButton.child, isA<Icon>());
      expect(ocrButton.leading, isNull);
      expect(find.text('OCR'), findsNothing);
      expect(find.bySemanticsLabel('Recognize image text'), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('history-ocr')));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.text('Recognizing image text.'), findsOneWidget);
      expect(
        tester
            .widget<Button>(find.byKey(const ValueKey('history-ocr-copy')))
            .onPressed,
        isNull,
      );
      final text = List.filled(80, 'Recognized screenshot text').join('\n');
      f.repository.pending!.complete(ModuleResult(text));
      await tester.pumpAndSettle();
      expect(find.text(text), findsOneWidget);
      expect(f.clipboard.values, isEmpty);
      await tester.tap(find.byKey(const ValueKey('history-ocr-copy')));
      await tester.pumpAndSettle();
      expect(f.clipboard.values, [text]);
      expect(find.text('Recognized text copied.'), findsOneWidget);
      // Advance the toast's normal auto-dismiss lifecycle before teardown.
      await tester.pump(const Duration(seconds: 6));
      await tester.pumpAndSettle();
      expect(controller.selectedId, 'screenshot');
      expect(controller.selectedClip?.contentKind, HistoryClipKind.image);
      expect(history.savedIds, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      await f.modules.setEnabled('copypaste.ocr', false);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('history-ocr')), findsNothing);
    }, variant: TargetPlatformVariant({platform}));
  }

  testWidgets('OCR failure can retry to an empty result', (tester) async {
    final f = await _fixture();
    addTearDown(f.modules.dispose);
    final controller = HistoryController(_HistoryRepository(), ocr: f.ocr);
    addTearDown(controller.dispose);
    f.repository.failure = const ModulesException('Model unavailable.');
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(child: HistoryScreen(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('history-clip-screenshot')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('history-ocr')));
    await tester.pumpAndSettle();
    expect(find.text('OCR failed'), findsOneWidget);
    expect(find.text('Model unavailable.'), findsOneWidget);
    f.repository.failure = null;
    f.repository.result = '';
    await tester.tap(find.text('Try again'));
    await tester.pumpAndSettle();
    expect(find.text('No text found'), findsOneWidget);
    expect(
      tester
          .widget<Button>(find.byKey(const ValueKey('history-ocr-copy')))
          .onPressed,
      isNull,
    );
    expect(f.input.cleaned, 2);
    expect(tester.takeException(), isNull);
  });
}

class _OcrModulesRepository extends MemoryModulesRepository {
  Completer<ModuleResult>? pending;
  String result = 'Recognized text';
  final invocations = <(String, String, String)>[];

  @override
  Future<ModuleResult> invoke(
    String id,
    String command,
    Map<String, Object> arguments,
  ) async {
    invocations.add((id, command, arguments['image_path'] as String));
    if (failure case final error?) throw error;
    return pending == null ? ModuleResult(result) : await pending!.future;
  }
}

class _ImageInput implements HistoryImageInput {
  final prepared = <String>[];
  int cleaned = 0;

  @override
  Future<SelectedModuleInput> prepare(HistoryClip clip) async {
    prepared.add(clip.id);
    return SelectedModuleInput(
      path: '/private/original.png',
      name: 'original.png',
      dispose: () async {
        cleaned++;
      },
    );
  }
}

class _Clipboard implements ClipboardWriter {
  final values = <String>[];

  @override
  Future<void> writeText(String text) async => values.add(text);
}

class _HistoryRepository implements HistoryRepository {
  final savedIds = <String>[];
  bool exportFails = false;

  @override
  Stream<HistoryRuntimeEvent> watch() => const Stream.empty();
  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();
  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async => HistoryClipPage(items: [_image]);
  @override
  Future<HistoryClip> get(String id) async => _image;
  @override
  Future<HistoryImagePreview?> imagePreview(
    String id, {
    int? maxEdge,
    HistoryImagePreviewBounds? bounds,
  }) async => null;
  @override
  Future<void> saveFile(String id, String destinationPath) async {
    savedIds.add(id);
    if (exportFails) throw StateError('Export failed');
    await File(destinationPath).writeAsBytes([1, 2, 3, 4]);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
