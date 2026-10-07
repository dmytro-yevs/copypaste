import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/onboarding/controller/windows_onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/repository/windows_onboarding_store.dart';
import 'package:copypaste_flutter/features/onboarding/view/windows_onboarding_screen.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  test('failed Windows persistence does not complete onboarding', () async {
    final controller = WindowsOnboardingController(store: _FailingStore());
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showReady();
    expect(await controller.finish(), isFalse);
    expect(controller.complete, isFalse);
    expect(controller.busy, isFalse);
    expect(controller.errorMessage, isNotNull);
  });
  test('Windows completion persists and cannot finish from Welcome', () async {
    final store = MemoryWindowsOnboardingStore();
    final first = WindowsOnboardingController(store: store);
    addTearDown(first.dispose);
    await first.initialize();
    expect(await first.finish(), isFalse);
    first.showReady();
    expect(await first.finish(), isTrue);
    final restarted = WindowsOnboardingController(store: store);
    addTearDown(restarted.dispose);
    await restarted.initialize();
    expect(restarted.complete, isTrue);
  });

  for (final width in [320.0, 768.0, 1024.0]) {
    for (final dark in [false, true]) {
      testWidgets(
        'Windows Focus fits $width with dark=$dark and one final action',
        (tester) async {
          await tester.binding.setSurfaceSize(Size(width, 640));
          addTearDown(() => tester.binding.setSurfaceSize(null));
          final controller = WindowsOnboardingController(
            store: MemoryWindowsOnboardingStore(),
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
          expect(find.text('CopyPaste is ready'), findsOneWidget);
          expect(find.text('Accessibility'), findsNothing);
          expect(find.byType(Button), findsOneWidget);
          expect(tester.takeException(), isNull);
          await tester.tap(find.widgetWithText(Button, 'Get started'));
          await tester.pumpAndSettle();
          expect(finished, isTrue);
        },
      );
    }
  }
}

class _FailingStore implements WindowsOnboardingStore {
  @override
  Future<bool> isComplete() async => false;

  @override
  Future<void> markComplete() async => throw StateError('Storage unavailable');
}
