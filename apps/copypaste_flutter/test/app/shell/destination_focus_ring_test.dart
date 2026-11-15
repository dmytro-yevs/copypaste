import 'dart:async';

import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/app_shell.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/history_repository.dart';
import 'package:copypaste_flutter/features/history/view/history_screen.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../features/settings/settings_test_support.dart';

class _Repository extends Fake implements HistoryRepository {
  final _events = StreamController<HistoryRuntimeEvent>.broadcast();

  @override
  Stream<HistoryRuntimeEvent> watch() => _events.stream;

  @override
  Future<HistoryFacets> facets() async => const HistoryFacets();

  @override
  Future<HistoryClipPage> query({
    required HistoryQuery query,
    required int limit,
    String? cursor,
  }) async => HistoryClipPage(
    items: [
      HistoryClip(
        id: 'clip',
        contentType: 'text/plain',
        preview: 'Clip',
        createdAt: DateTime.now(),
        pinned: false,
      ),
    ],
  );
}

/// The ring FocusOutline paints 3 px outside [control], in global coordinates,
/// and the part of it that every clipping ancestor leaves visible.
(Rect, Rect) _ringAndVisible(WidgetTester tester, Finder control) {
  final box = tester.renderObject<RenderBox>(
    find.descendant(of: control, matching: find.byType(FocusOutline)).first,
  );
  final ring = MatrixUtils.transformRect(
    box.getTransformTo(null),
    (Offset.zero & box.size).inflate(3),
  );
  var visible = ring;
  RenderObject child = box;
  for (var parent = box.parent; parent != null; parent = parent.parent) {
    final clip = parent.describeApproximatePaintClip(child);
    if (clip != null) {
      visible = visible.intersect(
        MatrixUtils.transformRect(parent.getTransformTo(null), clip),
      );
    }
    child = parent;
  }
  return (ring, visible);
}

void main() {
  testWidgets(
    'the History toolbar ring stays whole beside the open inspector',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1100, 760));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final history = HistoryController(_Repository());
      addTearDown(history.dispose);
      await history.initialize();
      await history.selection.select('clip');
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.dark,
          builder: AppTheme.builder,
          home: AppShell(
            controller: AppNavigationController(),
            unifiedTitleBar: true,
            destinations: {
              AppDestination.history: HistoryScreen(controller: history),
              AppDestination.devices: const SizedBox.expand(),
              AppDestination.settings: const SizedBox.expand(),
            },
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.byType(ResizablePanel), findsOneWidget);

      final search = find.byType(TextField).first;
      await tester.tap(search);
      await tester.pumpAndSettle();
      final (ring, visible) = _ringAndVisible(tester, search);
      expect(visible, ring);

      // The fix moves the clip, not the toolbar.
      final page = tester.getRect(find.byType(HistoryScreen));
      final field = tester.getRect(search);
      expect(field.top - page.top, AppSpacing.lg);

      // Header items outline themselves on focus instead of the shared ring.
      final item = find.byKey(const ValueKey('header-destination-history'));
      expect(Theme.of(tester.element(item)).colorScheme.ring.a, 0);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets('the Settings search ring stays whole under the header', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1100, 760));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final settings = SettingsController(
      screenshotProtection: FakeScreenshotProtection(),
      repository: FakeSettingsRepository(),
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    await settings.initialize();
    addTearDown(settings.dispose);
    final navigation = AppNavigationController(
      initialDestination: AppDestination.settings,
    );
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.dark,
        builder: AppTheme.builder,
        home: AppShell(
          controller: navigation,
          unifiedTitleBar: true,
          destinations: {
            AppDestination.history: const SizedBox.expand(),
            AppDestination.devices: const SizedBox.expand(),
            AppDestination.settings: SettingsScreen(controller: settings),
          },
        ),
      ),
    );
    await tester.pumpAndSettle();

    final search = find.byKey(const ValueKey<String>('settings-search'));
    await tester.tap(search);
    await tester.pumpAndSettle();
    final (ring, visible) = _ringAndVisible(tester, search);
    expect(visible, ring);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
}
