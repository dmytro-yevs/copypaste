import 'package:copypaste_flutter/app/app.dart';
import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('root navigates through the approved destinations', (
    tester,
  ) async {
    final navigation = AppNavigationController();
    addTearDown(navigation.dispose);
    await tester.pumpWidget(CopyPasteApp(navigation: navigation));
    await tester.pump();
    expect(find.text('History runtime is unavailable'), findsOneWidget);

    navigation.selectDestination(AppDestination.devices);
    await tester.pump();
    expect(find.text('Devices runtime is unavailable'), findsOneWidget);
    expect(find.text('History runtime is unavailable'), findsNothing);

    navigation.selectDestination(AppDestination.settings);
    await tester.pump();
    expect(find.text('Settings runtime is unavailable'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('runtime failure keeps the shell usable and retryable', (
    tester,
  ) async {
    final navigation = AppNavigationController();
    addTearDown(navigation.dispose);
    var retries = 0;

    await tester.pumpWidget(
      CopyPasteApp(
        navigation: navigation,
        runtimeUnavailableMessage: 'Protected history is unavailable.',
        onRetryRuntime: () async {
          retries += 1;
        },
      ),
    );
    await tester.pump();

    expect(find.text('History runtime is unavailable'), findsOneWidget);
    expect(find.text('Protected history is unavailable.'), findsOneWidget);
    await tester.tap(find.widgetWithText(Button, 'Retry'));
    await tester.pump();
    expect(retries, 1);

    navigation.selectDestination(AppDestination.devices);
    await tester.pump();
    expect(find.text('Devices runtime is unavailable'), findsOneWidget);

    navigation.selectDestination(AppDestination.settings);
    await tester.pump();
    expect(find.text('Settings runtime is unavailable'), findsOneWidget);
  });

  testWidgets(
    'application handles layout edges and large text in both themes',
    (tester) async {
      final navigation = AppNavigationController();
      addTearDown(navigation.dispose);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
      addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);

      for (final brightness in Brightness.values) {
        tester.platformDispatcher.platformBrightnessTestValue = brightness;
        for (final scale in [1.0, 1.3, 2.0]) {
          tester.platformDispatcher.textScaleFactorTestValue = scale;
          for (final width in [
            320.0,
            360.0,
            639.0,
            640.0,
            641.0,
            1023.0,
            1024.0,
            1440.0,
          ]) {
            await tester.binding.setSurfaceSize(Size(width, 480));
            await tester.pumpWidget(CopyPasteApp(navigation: navigation));
            await tester.pump();
            expect(
              tester.takeException(),
              isNull,
              reason: '$brightness, text scale $scale, width $width',
            );
            expect(find.text('History runtime is unavailable'), findsOneWidget);
          }
        }
      }
    },
  );
}
