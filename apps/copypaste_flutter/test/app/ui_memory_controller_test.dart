import 'package:copypaste_flutter/app/ui_memory_controller.dart';
import 'package:copypaste_flutter/platform/lifecycle/application_visibility.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('requires 60 continuous hidden seconds and restores on show', (
    tester,
  ) async {
    final visible = ValueNotifier(true);
    final controller = UiMemoryController(
      visible: visible,
      canSuspend: () => true,
    );
    addTearDown(controller.dispose);
    addTearDown(visible.dispose);
    visible.value = false;
    await tester.pump(const Duration(seconds: 59));
    expect(controller.suspended, isFalse);
    visible.value = true;
    await tester.pump(const Duration(seconds: 1));
    expect(controller.suspended, isFalse);
    visible.value = false;
    await tester.pump(const Duration(seconds: 59));
    expect(controller.suspended, isFalse);
    await tester.pump(const Duration(seconds: 1));
    expect(controller.suspended, isTrue);
    visible.value = true;
    expect(controller.suspended, isFalse);
  });

  testWidgets('preserves active operations until a later hidden deadline', (
    tester,
  ) async {
    final visible = ValueNotifier(false);
    var busy = true;
    final controller = UiMemoryController(
      visible: visible,
      canSuspend: () => !busy,
    );
    addTearDown(controller.dispose);
    addTearDown(visible.dispose);
    await tester.pump(const Duration(seconds: 60));
    expect(controller.suspended, isFalse);
    busy = false;
    await tester.pump(const Duration(seconds: 60));
    expect(controller.suspended, isTrue);
  });

  testWidgets('dispose cancels the hidden deadline', (tester) async {
    final visible = ValueNotifier(false);
    final controller = UiMemoryController(
      visible: visible,
      canSuspend: () => true,
    );
    var changes = 0;
    controller.addListener(() => changes++);
    controller.dispose();
    visible.value = true;
    await tester.pump(const Duration(seconds: 60));
    expect(changes, 0);
    visible.dispose();
  });

  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.android,
  ]) {
    testWidgets(
      'focus loss stays visible and backgrounding hides on $platform',
      (tester) async {
        tester.binding.handleAppLifecycleStateChanged(
          AppLifecycleState.resumed,
        );
        final window = ValueNotifier(true);
        final visibility = ApplicationVisibility(
          windowVisible: platform == TargetPlatform.android ? null : window,
        );
        addTearDown(visibility.dispose);
        addTearDown(window.dispose);
        addTearDown(
          () => tester.binding.handleAppLifecycleStateChanged(
            AppLifecycleState.resumed,
          ),
        );
        tester.binding.handleAppLifecycleStateChanged(
          AppLifecycleState.inactive,
        );
        expect(visibility.value, isTrue);
        tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
        expect(visibility.value, isFalse);
        if (platform == TargetPlatform.android) {
          tester.binding.handleAppLifecycleStateChanged(
            AppLifecycleState.paused,
          );
          expect(visibility.value, isFalse);
        } else {
          window.value = false;
          tester.binding.handleAppLifecycleStateChanged(
            AppLifecycleState.resumed,
          );
          expect(
            visibility.value,
            isFalse,
            reason: 'Quick Paste must not resume a minimized main window.',
          );
          window.value = true;
        }
        tester.binding.handleAppLifecycleStateChanged(
          AppLifecycleState.resumed,
        );
        expect(visibility.value, isTrue);
      },
      variant: TargetPlatformVariant({platform}),
    );
  }
}
