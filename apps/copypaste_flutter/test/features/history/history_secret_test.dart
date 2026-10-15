import 'dart:async';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_file_importer.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_inspector.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.android,
  ]) {
    testWidgets('eye action persists both secret states on $platform', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(const Size(440, 750));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _Repository();
      addTearDown(repository.events.close);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.select('clip');
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(
            child: AnimatedBuilder(
              animation: controller,
              builder: (context, _) =>
                  HistoryInspector(controller: controller, inDrawer: false),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final action = find.byKey(const ValueKey('history-detail-secret'));
      expect(tester.widget<Button>(action).child, isA<Icon>());
      expect(find.text('Synthetic private text'), findsOneWidget);
      await tester.tap(action);
      await tester.pump();
      expect(repository.clip.secret, isTrue);
      expect(controller.selectedClip!.secret, isTrue);
      expect(find.text('Synthetic private text'), findsNothing);
      expect(find.byIcon(LucideIcons.eye), findsOneWidget);
      await tester.tap(action);
      await tester.pumpAndSettle();
      expect(repository.clip.secret, isFalse);
      expect(find.text('Synthetic private text'), findsOneWidget);
      expect(find.byIcon(LucideIcons.eyeOff), findsOneWidget);
      await controller.select('clip');
      expect(controller.selectedClip!.secret, isFalse);
      expect(repository.choices, [true, false]);
      expect(tester.takeException(), isNull);
    }, variant: TargetPlatformVariant({platform}));

    for (final compact in [false, true]) {
      testWidgets(
        'remote secrecy conceals the open inspector on $platform, compact=$compact',
        (tester) async {
          await tester.binding.setSurfaceSize(const Size(440, 750));
          addTearDown(() => tester.binding.setSurfaceSize(null));
          final repository = _Repository();
          addTearDown(repository.events.close);
          final controller = HistoryController(repository);
          addTearDown(controller.dispose);
          await controller.initialize();
          await controller.select('clip');
          await tester.pumpWidget(
            ShadcnApp(
              theme: AppTheme.light,
              builder: AppTheme.builder,
              home: Scaffold(
                child: AnimatedBuilder(
                  animation: controller,
                  builder: (context, _) => HistoryInspector(
                    controller: controller,
                    inDrawer: false,
                    compact: compact,
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();
          expect(find.text('Synthetic private text'), findsOneWidget);

          repository.clip = repository.clip.copyWith(secret: true);
          repository.pendingGet = Completer<HistoryClip>();
          repository.events.add(HistoryRuntimeEvent.itemsChanged);
          await tester.pump();
          await tester.pump();

          expect(controller.items.single.secret, isTrue);
          expect(controller.selectedClip!.secret, isTrue);
          expect(find.text('Synthetic private text'), findsNothing);

          repository.pendingGet!.complete(repository.clip);
          repository.pendingGet = null;
          await tester.pump();
          expect(find.text('Synthetic private text'), findsNothing);
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox.shrink());
        },
        variant: TargetPlatformVariant({platform}),
      );
    }
  }

  test(
    'remote secrecy hides selected content removed from the query',
    () async {
      final repository = _Repository();
      addTearDown(repository.events.close);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.select('clip');
      repository.clip = repository.clip.copyWith(secret: true);
      repository.includeInPage = false;
      final pending = Completer<HistoryClip>();
      repository.pendingGet = pending;

      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);
      expect(controller.items, isEmpty);
      expect(controller.selectedClip, isNull);

      pending.complete(repository.clip);
      await Future<void>.delayed(Duration.zero);
      expect(controller.selectedClip!.secret, isTrue);
    },
  );

  test(
    'reload refreshes secrecy and restores newly public full content',
    () async {
      final repository = _Repository();
      addTearDown(repository.events.close);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.select('clip');
      repository.clip = repository.clip.copyWith(secret: true);
      await controller.reload();
      expect(controller.selectedClip!.secret, isTrue);

      repository.clip = repository.clip.copyWith(secret: false);
      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);
      expect(controller.selectedClip!.secret, isFalse);
      expect(controller.selectedClip!.body, 'Synthetic private text');
    },
  );

  test(
    'a deferred pinned refresh clears the inspector until secret detail loads',
    () async {
      final repository = _Repository()
        ..clip = _Repository.initialClip.copyWith(pinned: true)
        ..additionalClips.add(
          HistoryClip(
            id: 'second-pin',
            contentType: 'text',
            preview: 'Second pin',
            createdAt: DateTime.utc(2026),
            pinned: true,
          ),
        );
      addTearDown(repository.events.close);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.select('clip');
      controller.beginPinnedDrag('clip');
      expect(controller.draggedPinnedId, 'clip');

      repository.clip = repository.clip.copyWith(secret: true);
      final pending = Completer<HistoryClip>();
      repository.pendingGet = pending;
      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);
      expect(controller.selectedClip, isNull);

      pending.complete(repository.clip);
      await Future<void>.delayed(Duration.zero);
      expect(controller.selectedClip!.secret, isTrue);
      controller.endPinnedDrag();
    },
  );

  test('a deferred secret detail wins over an older selected read', () async {
    final repository = _Repository()
      ..clip = _Repository.initialClip.copyWith(pinned: true)
      ..additionalClips.add(
        HistoryClip(
          id: 'second-pin',
          contentType: 'text',
          preview: 'Second pin',
          createdAt: DateTime.utc(2026),
          pinned: true,
        ),
      );
    addTearDown(repository.events.close);
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await controller.initialize();

    final oldPublic = Completer<HistoryClip>();
    repository.getResponses.add(oldPublic.future);
    final selecting = controller.select('clip');
    controller.beginPinnedDrag('clip');
    repository.clip = repository.clip.copyWith(secret: true);
    final currentSecret = Completer<HistoryClip>();
    repository.getResponses.add(currentSecret.future);
    repository.events.add(HistoryRuntimeEvent.itemsChanged);
    await Future<void>.delayed(Duration.zero);

    currentSecret.complete(repository.clip);
    await Future<void>.delayed(Duration.zero);
    expect(controller.selectedClip!.secret, isTrue);

    oldPublic.complete(_Repository.initialClip.copyWith(pinned: true));
    await selecting;
    expect(controller.selectedClip!.secret, isTrue);
    controller.endPinnedDrag();
  });

  test(
    'a deferred file import clears the inspector until secret detail loads',
    () async {
      final repository = _Repository();
      final picked = Completer<List<HistoryImportFile>>();
      addTearDown(repository.events.close);
      final controller = HistoryController(
        repository,
        filePicker: _Picker(picked.future),
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.select('clip');
      final import = controller.chooseFiles();
      await Future<void>.delayed(Duration.zero);
      expect(controller.isImportingFiles, isTrue);

      repository.clip = repository.clip.copyWith(secret: true);
      final pending = Completer<HistoryClip>();
      repository.pendingGet = pending;
      repository.events.add(HistoryRuntimeEvent.itemsChanged);
      await Future<void>.delayed(Duration.zero);
      expect(controller.selectedClip, isNull);

      pending.complete(repository.clip);
      await Future<void>.delayed(Duration.zero);
      expect(controller.selectedClip!.secret, isTrue);
      repository.pendingGet = null;
      picked.complete(const []);
      await import;
    },
  );

  test(
    'secret mutation serializes repeats and retains state on failure',
    () async {
      final repository = _Repository();
      addTearDown(repository.events.close);
      final controller = HistoryController(repository);
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.select('clip');
      repository.pending = Completer<void>();
      final operation = controller.toggleSecret(controller.selectedClip!);
      expect(controller.isSecretPending('clip'), isTrue);
      expect(controller.canSuspend, isFalse);
      expect(await controller.toggleSecret(controller.selectedClip!), isFalse);
      repository.pending!.completeError(StateError('offline'));
      expect(await operation, isFalse);
      expect(controller.selectedClip!.secret, isFalse);
      expect(controller.isSecretPending('clip'), isFalse);
      expect(controller.errorMessage, isNotNull);
      repository.pending = null;
      expect(await controller.toggleSecret(controller.selectedClip!), isTrue);
      expect(controller.selectedClip!.secret, isTrue);
    },
  );

  test('a completed secret choice defeats a newer stale detail read', () async {
    final repository = _Repository();
    addTearDown(repository.events.close);
    final controller = HistoryController(repository);
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.select('clip');

    repository.pending = Completer<void>();
    final toggle = controller.toggleSecret(controller.selectedClip!);
    final stalePublic = Completer<HistoryClip>();
    repository.getResponses.add(stalePublic.future);
    final selecting = controller.select('clip');
    repository.pending!.complete();
    await toggle;
    expect(controller.selectedClip!.secret, isTrue);

    stalePublic.complete(_Repository.initialClip);
    await selecting;
    expect(controller.selectedClip!.secret, isTrue);
  });
}

class _Repository extends Fake implements HistoryRepository {
  static final initialClip = HistoryClip(
    id: 'clip',
    contentType: 'text',
    preview: 'Synthetic private text',
    body: 'Synthetic private text',
    createdAt: DateTime.utc(2026),
    pinned: false,
  );
  HistoryClip clip = _Repository.initialClip;
  final List<HistoryClip> additionalClips = [];
  final List<Future<HistoryClip>> getResponses = [];
  Completer<void>? pending;
  Completer<HistoryClip>? pendingGet;
  bool includeInPage = true;
  final events = StreamController<HistoryRuntimeEvent>.broadcast();
  final choices = <bool>[];
  @override
  Stream<HistoryRuntimeEvent> watch() => events.stream;
  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();
  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async =>
      HistoryClipPage(items: includeInPage ? [clip, ...additionalClips] : []);
  @override
  Future<HistoryClip> get(String id) async {
    if (getResponses.isNotEmpty) return await getResponses.removeAt(0);
    return pendingGet == null ? clip : await pendingGet!.future;
  }

  @override
  Future<HistoryClip> setSecret(String id, bool secret) async {
    choices.add(secret);
    await pending?.future;
    return clip = clip.copyWith(secret: secret);
  }
}

class _Picker implements HistoryFilePicker {
  const _Picker(this.files);

  final Future<List<HistoryImportFile>> files;

  @override
  Future<List<HistoryImportFile>> chooseFiles() => files;
}
