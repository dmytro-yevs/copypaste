import 'package:copypaste_flutter/features/onboarding/controller/android_onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/repository/android_onboarding_store.dart';
import 'package:copypaste_flutter/features/onboarding/view/android_onboarding_screen.dart';
import 'package:copypaste_flutter/platform/android/android_capture_setup_gateway.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('offers Full and Limited capture modes with six ADB commands', (
    tester,
  ) async {
    final controller = _controller();
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showCapture();

    await tester.pumpWidget(_app(controller));

    expect(find.text('Full capture'), findsOneWidget);
    expect(find.text('Background capture'), findsOneWidget);
    expect(find.text('Limited capture'), findsOneWidget);
    expect(find.text('Shizuku'), findsOneWidget);
    expect(find.text('ADB'), findsOneWidget);
    expect(find.text('Check again'), findsNothing);

    await controller.selectMethod(AndroidCaptureSetupMethod.adb);
    await tester.pump();
    expect(find.text('Check access'), findsNothing);
    expect(find.text('Check capture'), findsNothing);

    expect(find.text(_commands.join('\n')), findsOneWidget);
    final continueButton = tester.widget<Button>(
      find.widgetWithText(Button, 'Continue'),
    );
    expect(continueButton.onPressed, isNull);
    expect(
      tester.getSize(find.byType(Tabs)).width,
      tester.getSize(find.byType(RadioGroup<AndroidCaptureMode>)).width,
    );
    expect(
      tester.getCenter(find.byIcon(LucideIcons.bell)).dy,
      tester.getCenter(find.text('Capture notification')).dy,
    );
  });

  testWidgets('copies all six ADB commands as one multiline block', (
    tester,
  ) async {
    String? copiedText;
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        if (call.method == 'Clipboard.setData') {
          copiedText = (call.arguments as Map)['text'] as String;
        }
        return null;
      },
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        null,
      ),
    );
    final controller = _controller();
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showCapture();
    await controller.selectMethod(AndroidCaptureSetupMethod.adb);
    await tester.pumpWidget(_app(controller));

    expect(find.byType(SelectableText), findsOneWidget);
    final copy = find.byIcon(LucideIcons.copy);
    expect(copy, findsOneWidget);
    await tester.ensureVisible(copy);
    await tester.pumpAndSettle();
    await tester.tap(copy);
    await tester.pumpAndSettle();

    expect(copiedText, _commands.join('\n'));
    expect(find.byIcon(LucideIcons.copyCheck), findsOneWidget);
  });

  testWidgets('final step has one action and completes before opening home', (
    tester,
  ) async {
    final controller = _controller();
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showCapture();
    await controller.selectMode(AndroidCaptureMode.limited);
    await controller.continueFromCapture();
    var finished = false;
    await tester.pumpWidget(
      ShadcnApp(
        home: AndroidOnboardingScreen(
          controller: controller,
          onFinished: () async {
            finished = controller.complete;
          },
        ),
      ),
    );
    expect(find.byType(Button), findsOneWidget);
    expect(find.text('Open History'), findsNothing);
    expect(find.text('Pair a device'), findsNothing);
    await tester.tap(find.widgetWithText(Button, 'Get started'));
    await tester.pumpAndSettle();
    expect(finished, isTrue);
  });

  testWidgets('Limited mode reaches Sync without privileged setup', (
    tester,
  ) async {
    final controller = _controller();
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showCapture();
    await controller.selectMode(AndroidCaptureMode.limited);

    await tester.pumpWidget(_app(controller));
    await tester.tap(find.widgetWithText(Button, 'Continue'));
    await tester.pump();

    expect(find.text('CopyPaste is ready'), findsOneWidget);
  });

  testWidgets('fits a narrow phone with enlarged text', (tester) async {
    addTearDown(() => tester.binding.setSurfaceSize(null));
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    await tester.binding.setSurfaceSize(const Size(320, 640));
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    final controller = _controller();
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showCapture();

    await tester.pumpWidget(_app(controller));
    expect(tester.takeException(), isNull);

    await controller.selectMode(AndroidCaptureMode.limited);
    await tester.pump();
    expect(tester.takeException(), isNull);

    await controller.continueFromCapture();
    await tester.pump();
    expect(tester.takeException(), isNull);
  });

  testWidgets('uses no outline button variants', (tester) async {
    final controller = _controller();
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showCapture();

    await tester.pumpWidget(_app(controller));

    expect(find.byType(OutlineButton), findsNothing);
  });

  testWidgets('reopened setup displays the running Full capture state', (
    tester,
  ) async {
    final setup = _ScreenAndroidCaptureSetupGateway()
      ..current = const AndroidCaptureSetupState(
        packageName: 'com.copypaste.app',
        privilegedGrants: true,
        notificationGranted: true,
        batteryExempt: true,
        captureEnabled: true,
        serviceRunning: true,
        lastCaptureAtMs: 21,
        shizuku: AndroidShizukuState(
          supported: true,
          installed: false,
          running: false,
          permission: false,
        ),
        adbCommands: _commands,
      );
    final controller = AndroidOnboardingController(
      store: MemoryAndroidOnboardingStore(
        complete: true,
        mode: AndroidCaptureMode.limited,
        verificationBaseline: 20,
      ),
      setup: setup,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.reopenCaptureSetup();

    await tester.pumpWidget(_app(controller));

    expect(find.text('Full capture'), findsNWidgets(2));
    expect(find.text('Background capture is working'), findsOneWidget);
    expect(
      tester
          .widget<RadioGroup<AndroidCaptureMode>>(
            find.byType(RadioGroup<AndroidCaptureMode>),
          )
          .value,
      AndroidCaptureMode.full,
    );
    expect(
      tester.widget<Button>(find.widgetWithText(Button, 'Continue')).onPressed,
      isNotNull,
    );
  });
}

const _commands = [
  'adb shell pm grant com.copypaste.app android.permission.READ_LOGS',
  'adb shell cmd appops set com.copypaste.app SYSTEM_ALERT_WINDOW allow',
  'adb shell cmd appops set com.copypaste.app RUN_IN_BACKGROUND allow',
  'adb shell cmd appops set com.copypaste.app RUN_ANY_IN_BACKGROUND allow',
  'adb shell am set-inactive com.copypaste.app false',
  'adb shell am set-standby-bucket com.copypaste.app active',
];

AndroidOnboardingController _controller() => AndroidOnboardingController(
  store: MemoryAndroidOnboardingStore(),
  setup: _ScreenAndroidCaptureSetupGateway(),
);

Widget _app(AndroidOnboardingController controller) => ShadcnApp(
  home: AndroidOnboardingScreen(
    controller: controller,
    onFinished: () async {},
  ),
);

class _ScreenAndroidCaptureSetupGateway implements AndroidCaptureSetupGateway {
  @override
  Stream<AndroidCaptureSetupState> get changes => const Stream.empty();

  AndroidCaptureSetupState current = const AndroidCaptureSetupState(
    packageName: 'com.copypaste.app',
    privilegedGrants: false,
    notificationGranted: false,
    batteryExempt: false,
    captureEnabled: false,
    serviceRunning: false,
    lastCaptureAtMs: 0,
    shizuku: AndroidShizukuState(
      supported: true,
      installed: false,
      running: false,
      permission: false,
    ),
    adbCommands: _commands,
  );

  @override
  Future<AndroidCaptureSetupState> applyShizukuGrants() async => current;

  @override
  Future<bool> openShizuku() async => true;

  @override
  Future<bool> requestBatteryExemption() async => true;

  @override
  Future<AndroidCaptureSetupState> requestNotifications() async => current;

  @override
  Future<bool> setForegroundCaptureEnabled(bool enabled) async => true;

  @override
  Future<AndroidCaptureSetupState> startCapture() async => current;

  @override
  Future<AndroidCaptureSetupState> state() async => current;

  @override
  Future<AndroidCaptureSetupState> stopCapture() async => current;
}
