import 'dart:async';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
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
  }

  test(
    'secret mutation serializes repeats and retains state on failure',
    () async {
      final repository = _Repository();
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
}

class _Repository extends Fake implements HistoryRepository {
  var clip = HistoryClip(
    id: 'clip',
    contentType: 'text',
    preview: 'Synthetic private text',
    body: 'Synthetic private text',
    createdAt: DateTime.utc(2026),
    pinned: false,
  );
  Completer<void>? pending;
  final choices = <bool>[];
  @override
  Stream<HistoryRuntimeEvent> watch() => const Stream.empty();
  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();
  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async => HistoryClipPage(items: [clip]);
  @override
  Future<HistoryClip> get(String id) async => clip;
  @override
  Future<HistoryClip> setSecret(String id, bool secret) async {
    choices.add(secret);
    await pending?.future;
    return clip = clip.copyWith(secret: secret);
  }
}
