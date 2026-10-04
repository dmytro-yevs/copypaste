import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  test(
    'starts on History and switches top-level destinations without routes',
    () {
      final AppNavigationController controller = AppNavigationController();
      var notifications = 0;
      controller.addListener(() => notifications++);

      expect(controller.selectedDestination, AppDestination.history);
      expect(controller.navigatorKey.currentState, isNull);

      controller.selectDestination(AppDestination.devices);

      expect(controller.selectedDestination, AppDestination.devices);
      expect(notifications, 1);

      controller.selectDestination(AppDestination.devices);

      expect(notifications, 1);
    },
  );

  test('shares labels, icons, and selected semantics', () {
    final AppNavigationDestination history =
        AppDestination.history.navigationDestination;

    expect(appNavigationDestinations, hasLength(3));
    expect(history.label, 'History');
    expect(history.icon, isNotNull);
    expect(history.semanticsLabel(selected: true), 'History, selected');
    expect(history.semanticsLabel(selected: false), 'History');
  });

  test('tracks bottom overlay visibility without duplicate notifications', () {
    final controller = AppNavigationController();
    var notifications = 0;
    controller.addListener(() => notifications++);

    controller.setBottomOverlayOpen(true);
    controller.setBottomOverlayOpen(true);

    expect(controller.bottomOverlayOpen, isTrue);
    expect(notifications, 1);

    controller.setBottomOverlayOpen(false);

    expect(controller.bottomOverlayOpen, isFalse);
    expect(notifications, 2);
  });

  testWidgets('pops the top nested route before reaching the root', (
    WidgetTester tester,
  ) async {
    final AppNavigationController controller = AppNavigationController();

    await tester.pumpWidget(_NavigationHarness(controller: controller));
    await tester.pumpAndSettle();

    controller.push<void>(
      tester.element(find.text('Root')),
      builder: (BuildContext context) => const Scaffold(child: Text('Detail')),
    );
    await tester.pumpAndSettle();

    expect(find.text('Detail'), findsOneWidget);
    expect(await controller.handleEscape(), isTrue);
    await tester.pumpAndSettle();

    expect(find.text('Root'), findsOneWidget);
    expect(await controller.handleEscape(), isFalse);
  });

  testWidgets('switching destinations returns nested navigation to its root', (
    WidgetTester tester,
  ) async {
    final AppNavigationController controller = AppNavigationController();

    await tester.pumpWidget(_NavigationHarness(controller: controller));
    await tester.pumpAndSettle();

    controller.push<void>(
      tester.element(find.text('Root')),
      builder: (BuildContext context) => const Scaffold(child: Text('Detail')),
    );
    await tester.pumpAndSettle();

    controller.selectDestination(AppDestination.devices);
    await tester.pumpAndSettle();

    expect(controller.selectedDestination, AppDestination.devices);
    expect(find.text('Root'), findsOneWidget);
    expect(find.text('Detail'), findsNothing);
  });

  testWidgets('updates the root child without replacing nested navigation', (
    WidgetTester tester,
  ) async {
    final AppNavigationController controller = AppNavigationController();

    await tester.pumpWidget(
      _NavigationHarness(controller: controller, rootLabel: 'History'),
    );
    await tester.pumpAndSettle();

    controller.push<void>(
      tester.element(find.text('History')),
      builder: (BuildContext context) => const Scaffold(child: Text('Detail')),
    );
    await tester.pumpAndSettle();

    await tester.pumpWidget(
      _NavigationHarness(controller: controller, rootLabel: 'Devices'),
    );
    await tester.pumpAndSettle();

    expect(find.text('Detail'), findsOneWidget);
    expect(await controller.maybePop(), isTrue);
    await tester.pumpAndSettle();

    expect(find.text('Devices'), findsOneWidget);
    expect(find.text('History'), findsNothing);
  });

  testWidgets('creates a route with no animation when motion is disabled', (
    WidgetTester tester,
  ) async {
    final AppPageRoute<void> route = AppPageRoute<void>(
      disableAnimations: true,
      builder: (BuildContext context) => const SizedBox.shrink(),
    );

    expect(route.transitionDuration, Duration.zero);
    expect(route.reverseTransitionDuration, Duration.zero);
  });

  test('creates a route with the shared emphasized duration', () {
    final AppPageRoute<void> route = AppPageRoute<void>(
      disableAnimations: false,
      builder: (BuildContext context) => const SizedBox.shrink(),
    );

    expect(route.transitionDuration, AppMotion.emphasized);
    expect(route.reverseTransitionDuration, AppMotion.emphasized);
  });
}

class _NavigationHarness extends StatelessWidget {
  const _NavigationHarness({required this.controller, this.rootLabel = 'Root'});

  final AppNavigationController controller;
  final String rootLabel;

  @override
  Widget build(BuildContext context) {
    return ShadcnApp(
      home: MediaQuery(
        data: const MediaQueryData(),
        child: AppNavigationHost(
          controller: controller,
          child: Scaffold(child: Text(rootLabel)),
        ),
      ),
    );
  }
}
