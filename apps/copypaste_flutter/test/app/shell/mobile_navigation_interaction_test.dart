import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/shell.dart';
import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

const _platforms = TargetPlatformVariant({
  TargetPlatform.android,
  TargetPlatform.macOS,
  TargetPlatform.windows,
});

void main() {
  testWidgets('keeps Telegram geometry independent of mobile theme scaling', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(controller));
    final surface = find.byKey(const ValueKey('mobile-navigation-surface'));
    expect(tester.getSize(surface), const Size(328, 56));
    for (final item in appNavigationDestinations) {
      final tab = _tab(item.destination);
      final icon = find.descendant(of: tab, matching: find.byType(Icon));
      final text = find.descendant(of: tab, matching: find.text(item.label));
      expect(tester.getSize(tab).height, 48);
      expect(tester.getSize(icon), const Size(24, 24));
      expect(
        tester.renderObject<RenderParagraph>(text).text.style!.fontSize,
        12,
      );
    }
    expect(tester.takeException(), isNull);
  }, variant: _platforms);

  testWidgets(
    'swipes between destinations, retains scroll, and reselects to top',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 720));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = AppNavigationController();
      addTearDown(controller.dispose);
      await tester.pumpWidget(_shell(controller));
      final history = controller.scrollControllerFor(AppDestination.history);
      history.jumpTo(350);
      await tester.pump();
      await tester.drag(
        find.byKey(const ValueKey('destination-pager')),
        const Offset(-300, 0),
      );
      await tester.pumpAndSettle();
      expect(controller.selectedDestination, AppDestination.devices);
      expect(controller.visualPosition.value, closeTo(1, 0.001));
      await tester.drag(
        find.byKey(const ValueKey('destination-pager')),
        const Offset(300, 0),
      );
      await tester.pumpAndSettle();
      expect(controller.selectedDestination, AppDestination.history);
      expect(history.offset, closeTo(350, 0.1));
      await tester.tap(_tab(AppDestination.history));
      await tester.pumpAndSettle();
      expect(history.offset, 0);
      expect(tester.takeException(), isNull);
    },
    variant: _platforms,
  );

  testWidgets('holds and drags the selector before committing on release', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(controller));
    final gesture = await tester.startGesture(
      tester.getCenter(_tab(AppDestination.history)),
    );
    await tester.pump(
      AppMotion.navigationHold + const Duration(milliseconds: 1),
    );
    final indicator = find.byKey(
      const ValueKey('mobile-navigation-drag-indicator'),
    );
    expect(indicator, findsOneWidget);
    await gesture.moveTo(tester.getCenter(_tab(AppDestination.settings)));
    await tester.pump();
    expect(controller.selectedDestination, AppDestination.history);
    expect(
      tester.getCenter(indicator).dx,
      closeTo(tester.getCenter(_tab(AppDestination.settings)).dx, 1),
    );
    await gesture.up();
    await tester.pumpAndSettle();
    expect(controller.selectedDestination, AppDestination.settings);
    expect(controller.visualPosition.value, closeTo(2, 0.001));
    expect(tester.takeException(), isNull);
  }, variant: _platforms);

  testWidgets('cancelling a hold restores the current destination', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(controller));
    final gesture = await tester.startGesture(
      tester.getCenter(_tab(AppDestination.history)),
    );
    await tester.pump(
      AppMotion.navigationHold + const Duration(milliseconds: 1),
    );
    await gesture.moveTo(tester.getCenter(_tab(AppDestination.settings)));
    await gesture.cancel();
    await tester.pumpAndSettle();
    expect(controller.selectedDestination, AppDestination.history);
    expect(controller.visualPosition.value, 0);
    expect(tester.takeException(), isNull);
  }, variant: _platforms);

  testWidgets('reselection pops details and reduced motion jumps directly', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(controller, reducedMotion: true));
    await tester.tap(_tab(AppDestination.settings));
    await tester.pump();
    expect(controller.selectedDestination, AppDestination.settings);
    expect(controller.visualPosition.value, 2);
    controller.push<void>(
      tester.element(find.byType(AppBar)),
      builder: (_) => const Text('Detail route'),
    );
    await tester.pumpAndSettle();
    expect(find.text('Detail route'), findsOneWidget);
    await tester.tap(_tab(AppDestination.settings));
    await tester.pumpAndSettle();
    expect(find.text('Detail route'), findsNothing);
    expect(tester.takeException(), isNull);
  }, variant: _platforms);
  testWidgets('system Back pops details before returning to History', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(controller));
    await tester.tap(_tab(AppDestination.devices));
    await tester.pumpAndSettle();
    controller.push<void>(
      tester.element(find.byType(AppBar)),
      builder: (_) => const Text('Device detail'),
    );
    await tester.pumpAndSettle();
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(find.text('Device detail'), findsNothing);
    expect(controller.selectedDestination, AppDestination.devices);
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(controller.selectedDestination, AppDestination.history);
    expect(tester.takeException(), isNull);
  }, variant: _platforms);
  testWidgets('closing the bar during selector settlement cancels its motion', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(controller));
    final gesture = await tester.startGesture(
      tester.getCenter(_tab(AppDestination.history)),
    );
    await tester.pump(
      AppMotion.navigationHold + const Duration(milliseconds: 1),
    );
    await gesture.cancel();
    controller.setBottomOverlayOpen(true);
    await tester.pump();
    expect(find.byType(NavigationBar), findsNothing);
    expect(tester.takeException(), isNull);
  }, variant: _platforms);
  testWidgets(
    'tap selection skips intermediate tabs while page swipes track the finger',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 720));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = AppNavigationController();
      addTearDown(controller.dispose);
      await tester.pumpWidget(_shell(controller));
      await tester.tap(_tab(AppDestination.settings));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(controller.visualPosition.value, greaterThan(0.5));
      expect(controller.pageGestureActive.value, isFalse);
      final middle = tester.widget<DecoratedBox>(
        find.byKey(const ValueKey('mobile-navigation-selection-1')),
      );
      expect((middle.decoration as BoxDecoration).color!.a, 0);
      await tester.pumpAndSettle();
      final gesture = await tester.startGesture(
        tester.getCenter(find.byKey(const ValueKey('destination-pager'))),
      );
      await gesture.moveBy(const Offset(40, 0));
      await gesture.moveBy(const Offset(80, 0));
      await tester.pump();
      expect(controller.pageGestureActive.value, isTrue);
      expect(controller.visualPosition.value, inExclusiveRange(1, 2));
      await gesture.up();
      await tester.pumpAndSettle();
      expect(controller.pageGestureActive.value, isFalse);
      expect(tester.takeException(), isNull);
    },
    variant: _platforms,
  );
  testWidgets(
    'a programmatic activation supersedes the pending page transition',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 720));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final controller = AppNavigationController();
      addTearDown(controller.dispose);
      await tester.pumpWidget(_shell(controller));
      controller.selectDestination(AppDestination.settings);
      controller.selectDestination(AppDestination.history);
      await tester.pumpAndSettle();
      expect(controller.selectedDestination, AppDestination.history);
      expect(controller.visualPosition.value, 0);
      expect(tester.takeException(), isNull);
    },
    variant: _platforms,
  );
  testWidgets('mouse input can swipe the compact viewport', (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(controller));
    final gesture = await tester.startGesture(
      tester.getCenter(find.byKey(const ValueKey('destination-pager'))),
      kind: PointerDeviceKind.mouse,
    );
    await gesture.moveBy(const Offset(-40, 0));
    await gesture.moveBy(const Offset(-260, 0));
    await gesture.up();
    await tester.pumpAndSettle();
    expect(controller.selectedDestination, AppDestination.devices);
    expect(tester.takeException(), isNull);
  }, variant: _platforms);

  testWidgets('footer clearance includes the system safe area only once', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);
    double? clearance;
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: MediaQuery(
          data: const MediaQueryData(
            padding: EdgeInsets.only(bottom: 24),
            viewPadding: EdgeInsets.only(bottom: 24),
          ),
          child: AppShell(
            controller: controller,
            destinations: {
              AppDestination.history: Builder(
                builder: (context) {
                  clearance = MediaQuery.paddingOf(context).bottom;
                  return const SizedBox.expand();
                },
              ),
              AppDestination.devices: const SizedBox.expand(),
              AppDestination.settings: const SizedBox.expand(),
            },
          ),
        ),
      ),
    );
    await tester.pump();
    expect(clearance, closeTo(56 + 16 + 24, 0.001));
    controller.setBottomOverlayOpen(true);
    await tester.pump();
    expect(clearance, 24);
    expect(tester.takeException(), isNull);
  }, variant: _platforms);
  testWidgets('tab clicks wait until the current page transition ends', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 720));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AppNavigationController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(controller));
    await tester.tap(_tab(AppDestination.settings));
    await tester.tap(_tab(AppDestination.history));
    await tester.pumpAndSettle();
    expect(controller.selectedDestination, AppDestination.settings);
    expect(controller.visualPosition.value, 2);
    await tester.tap(_tab(AppDestination.history));
    await tester.pumpAndSettle();
    expect(controller.selectedDestination, AppDestination.history);
    expect(controller.visualPosition.value, 0);
    expect(tester.takeException(), isNull);
  }, variant: _platforms);
}

Finder _tab(AppDestination destination) => find.descendant(
  of: find.byKey(const ValueKey('bottom-navigation')),
  matching: find.byKey(ValueKey(destination)),
);

Widget _shell(
  AppNavigationController controller, {
  bool reducedMotion = false,
}) => ShadcnApp(
  theme: AppTheme.light,
  darkTheme: AppTheme.dark,
  themeMode: ThemeMode.dark,
  builder: AppTheme.builder,
  home: MediaQuery(
    data: MediaQueryData(disableAnimations: reducedMotion),
    child: AppShell(
      controller: controller,
      destinations: {
        for (final item in appNavigationDestinations)
          item.destination: ListView(
            controller: controller.scrollControllerFor(item.destination),
            children: [
              for (var index = 0; index < 30; index++)
                Padding(
                  padding: const EdgeInsets.all(AppSpacing.xxl),
                  child: Text('${item.label} row $index'),
                ),
            ],
          ),
      },
    ),
  ),
);
