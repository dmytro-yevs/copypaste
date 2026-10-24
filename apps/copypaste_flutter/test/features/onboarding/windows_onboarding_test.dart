import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/onboarding/controller/windows_onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/repository/onboarding_store.dart';
import 'package:copypaste_flutter/features/onboarding/view/windows_onboarding_screen.dart';
import 'package:copypaste_flutter/platform/startup/launch_at_login.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  test('failed Windows persistence does not complete onboarding', () async {
    final controller = WindowsOnboardingController(
      store: _FailingStore(),
      startup: _Startup(),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showSetup();
    await controller.continueFromSetup();
    expect(await controller.finish(), isFalse);
    expect(controller.complete, isFalse);
    expect(controller.busy, isFalse);
    expect(controller.errorMessage, isNotNull);
  });
  test('Windows completion persists and cannot finish from Welcome', () async {
    final store = MemoryOnboardingStore();
    final startup = _Startup();
    final first = WindowsOnboardingController(store: store, startup: startup);
    addTearDown(first.dispose);
    await first.initialize();
    expect(await first.finish(), isFalse);
    expect(first.launchAtLogin, isTrue);
    expect(startup.writes, isEmpty);
    first.showSetup();
    expect(await first.finish(), isFalse);
    await first.continueFromSetup();
    expect(startup.writes, [true]);
    expect(await first.finish(), isTrue);
    final untouched = _Startup()..status = LaunchAtLoginStatus.disabled;
    final restarted = WindowsOnboardingController(
      store: store,
      startup: untouched,
    );
    addTearDown(restarted.dispose);
    await restarted.initialize();
    expect(restarted.complete, isTrue);
    expect(untouched.reads, 0);
    expect(untouched.writes, isEmpty);
  });

  test(
    'fresh startup opt-out survives Back and does not enable startup',
    () async {
      final startup = _Startup();
      final controller = WindowsOnboardingController(
        store: MemoryOnboardingStore(),
        startup: startup,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      controller.showSetup();
      await controller.setLaunchAtLogin(false);
      controller.showPreviousStep();
      expect(controller.step, WindowsOnboardingStep.welcome);
      controller.showSetup();
      expect(controller.launchAtLogin, isFalse);
      await controller.continueFromSetup();
      controller.showPreviousStep();
      expect(controller.step, WindowsOnboardingStep.setup);
      expect(controller.launchAtLogin, isFalse);
      expect(startup.writes, isEmpty);
    },
  );

  test(
    'already enabled startup is preserved without a redundant write',
    () async {
      final startup = _Startup()..status = LaunchAtLoginStatus.enabled;
      final controller = WindowsOnboardingController(
        store: MemoryOnboardingStore(),
        startup: startup,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      controller.showSetup();
      await controller.continueFromSetup();
      expect(controller.launchAtLogin, isTrue);
      expect(startup.writes, isEmpty);
    },
  );

  test('opting out disables an existing startup registration', () async {
    final startup = _Startup()..status = LaunchAtLoginStatus.enabled;
    final controller = WindowsOnboardingController(
      store: MemoryOnboardingStore(),
      startup: startup,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showSetup();
    await controller.setLaunchAtLogin(false);
    await controller.continueFromSetup();
    expect(controller.launchAtLogin, isFalse);
    expect(startup.writes, [false]);
  });

  test(
    'startup update failure is optional and completion still persists',
    () async {
      final startup = _Startup()..failWrite = true;
      final store = MemoryOnboardingStore();
      final controller = WindowsOnboardingController(
        store: store,
        startup: startup,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      controller.showSetup();
      expect(await controller.continueFromSetup(), isTrue);
      expect(controller.launchAtLogin, isFalse);
      expect(controller.noticeMessage, contains('continue without it'));
      expect(await controller.finish(), isTrue);
      expect(await store.isComplete(), isTrue);
    },
  );

  for (final failedRead in [false, true]) {
    test(
      'unavailable startup does not block completion (read failure=$failedRead)',
      () async {
        final startup = _Startup()
          ..status = LaunchAtLoginStatus.unavailable
          ..failRead = failedRead;
        final controller = WindowsOnboardingController(
          store: MemoryOnboardingStore(),
          startup: startup,
        );
        addTearDown(controller.dispose);
        await controller.initialize();
        expect(controller.launchAtLoginAvailable, isFalse);
        expect(controller.launchAtLogin, isFalse);
        controller.showSetup();
        expect(await controller.continueFromSetup(), isTrue);
        expect(await controller.finish(), isTrue);
        expect(startup.writes, isEmpty);
      },
    );
  }

  testWidgets('unavailable startup is disabled and leaves Continue usable', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(320, 640));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final startup = _Startup()..status = LaunchAtLoginStatus.unavailable;
    final controller = WindowsOnboardingController(
      store: MemoryOnboardingStore(),
      startup: startup,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showSetup();
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.dark,
        builder: AppTheme.builder,
        home: WindowsOnboardingScreen(
          controller: controller,
          onFinished: () async {},
        ),
      ),
    );
    expect(tester.widget<Switch>(find.byType(Switch)).onChanged, isNull);
    expect(
      find.textContaining('Unavailable in this installation'),
      findsOneWidget,
    );
    await tester.tap(find.widgetWithText(Button, 'Continue'));
    await tester.pumpAndSettle();
    expect(find.text('CopyPaste is ready'), findsOneWidget);
    expect(tester.takeException(), isNull);
    expect(startup.writes, isEmpty);
  });

  for (final width in [320.0, 768.0, 1024.0]) {
    for (final dark in [false, true]) {
      testWidgets(
        'Windows Focus fits $width with dark=$dark and one final action',
        (tester) async {
          await tester.binding.setSurfaceSize(Size(width, 640));
          addTearDown(() => tester.binding.setSurfaceSize(null));
          final controller = WindowsOnboardingController(
            store: MemoryOnboardingStore(),
            startup: _Startup(),
          );
          addTearDown(controller.dispose);
          await controller.initialize();
          var finished = false;
          await tester.pumpWidget(
            ShadcnApp(
              theme: dark ? AppTheme.dark : AppTheme.light,
              builder: AppTheme.builder,
              home: WindowsOnboardingScreen(
                controller: controller,
                onFinished: () async {
                  finished = controller.complete;
                },
              ),
            ),
          );
          expect(find.text('Welcome to CopyPaste'), findsOneWidget);
          expect(tester.takeException(), isNull);
          await tester.tap(find.widgetWithText(Button, 'Continue'));
          await tester.pumpAndSettle();
          expect(find.text('Start at login'), findsOneWidget);
          expect(find.text('Accessibility'), findsNothing);
          expect(find.byType(Switch), findsOneWidget);
          expect(tester.widget<Switch>(find.byType(Switch)).value, isTrue);
          expect(tester.takeException(), isNull);
          await tester.tap(find.byType(Switch));
          await tester.pumpAndSettle();
          expect(tester.widget<Switch>(find.byType(Switch)).value, isFalse);
          await tester.tap(find.widgetWithText(Button, 'Back'));
          await tester.pumpAndSettle();
          expect(find.text('Welcome to CopyPaste'), findsOneWidget);
          await tester.tap(find.widgetWithText(Button, 'Continue'));
          await tester.pumpAndSettle();
          expect(tester.widget<Switch>(find.byType(Switch)).value, isFalse);
          await tester.tap(find.widgetWithText(Button, 'Continue'));
          await tester.pumpAndSettle();
          expect(find.text('CopyPaste is ready'), findsOneWidget);
          expect(find.text('Accessibility'), findsNothing);
          expect(find.widgetWithText(Button, 'Get started'), findsOneWidget);
          expect(tester.takeException(), isNull);
          await tester.tap(find.widgetWithText(Button, 'Get started'));
          await tester.pumpAndSettle();
          expect(finished, isTrue);
        },
      );
    }
  }
}

class _Startup implements LaunchAtLoginGateway {
  LaunchAtLoginStatus status = LaunchAtLoginStatus.disabled;
  final writes = <bool>[];
  int reads = 0;
  bool failRead = false;
  bool failWrite = false;
  @override
  Future<LaunchAtLoginStatus> read() async {
    reads++;
    if (failRead) throw StateError('Startup unavailable');
    return status;
  }

  @override
  Future<LaunchAtLoginStatus> setEnabled(bool enabled) async {
    writes.add(enabled);
    if (failWrite) throw StateError('Startup denied');
    return status = enabled
        ? LaunchAtLoginStatus.enabled
        : LaunchAtLoginStatus.disabled;
  }

  @override
  Future<void> openSystemSettings() async {}
}

class _FailingStore implements OnboardingStore {
  @override
  Future<bool> isComplete() async => false;

  @override
  Future<void> markComplete() async => throw StateError('Storage unavailable');
}
