import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/devices/device_presentation.dart';
import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/shared/step_progress.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  Future<void> pumpIndicator(
    WidgetTester tester,
    PairingProgress progress, {
    required double width,
    bool dark = false,
  }) => tester.pumpWidget(
    ShadcnApp(
      theme: dark ? AppTheme.dark : AppTheme.light,
      builder: AppTheme.builder,
      home: Scaffold(
        child: Center(
          child: SizedBox(
            width: width,
            child: DevicePresentation.pairingStepProgress(progress),
          ),
        ),
      ),
    ),
  );

  FontWeight? weightOf(WidgetTester tester, String text) => tester
      .widget<RichText>(
        find.descendant(of: find.text(text), matching: find.byType(RichText)),
      )
      .text
      .style
      ?.fontWeight;

  testWidgets('names every step and marks the current one beyond colour', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      await pumpIndicator(
        tester,
        PairingProgress.resolve(
          mode: PairingEntryMode.scanQr,
          state: PairingState.handshaking,
        ),
        width: 640,
      );
      for (final name in ['Scan', 'Connect', 'Confirm', 'Paired']) {
        expect(find.text(name), findsOneWidget);
      }
      // Done shows a check, current keeps its number and a heavier name.
      expect(find.byIcon(LucideIcons.check), findsOneWidget);
      expect(find.text('2'), findsOneWidget);
      expect(weightOf(tester, 'Connect'), FontWeight.w600);
      expect(weightOf(tester, 'Confirm'), isNot(FontWeight.w600));
      // Each pairing state shows its own activity; the steps only orient.
      expect(find.byType(LinearProgressIndicator), findsNothing);
      expect(find.semantics.byLabel('Step 2 of 4: Connect'), findsOneWidget);

      await pumpIndicator(
        tester,
        PairingProgress.resolve(
          mode: PairingEntryMode.scanQr,
          state: PairingState.confirmed,
        ),
        width: 640,
      );
      await tester.pumpAndSettle();
      expect(find.byIcon(LucideIcons.check), findsNWidgets(4));
      expect(
        tester.getSemantics(find.byType(StepProgress)),
        isSemantics(label: 'Step 4 of 4: Paired', value: ''),
      );
      expect(tester.takeException(), isNull);
    } finally {
      semantics.dispose();
    }
  });

  for (final dark in [false, true]) {
    testWidgets(
      'shortens to the current step at 280 pixels with doubled text, dark=$dark',
      (tester) async {
        tester.platformDispatcher.textScaleFactorTestValue = 2.0;
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        AppStrings.useLocale(const Locale('de'));
        addTearDown(() => AppStrings.useLocale(AppLocalization.fallback));
        final semantics = tester.ensureSemantics();
        try {
          await pumpIndicator(
            tester,
            PairingProgress.resolve(
              mode: PairingEntryMode.enterCode,
              state: PairingState.rejected,
              furthestState: PairingState.awaitingConfirmation,
            ),
            width: 280,
            dark: dark,
          );
          expect(find.textContaining('Bestätigung'), findsOneWidget);
          expect(find.text('Verbindung'), findsNothing);
          // Two steps happened, the third failed, the last was never reached.
          expect(find.byIcon(LucideIcons.check), findsNWidgets(2));
          expect(find.byIcon(LucideIcons.x), findsOneWidget);
          expect(find.text('4'), findsOneWidget);
          // This device did not reject, so the outcome names no party.
          expect(
            tester.getSemantics(find.byType(StepProgress)),
            isSemantics(
              label: 'Schritt 3 von 4: Bestätigung',
              value: 'Kopplung nicht abgeschlossen',
            ),
          );
          final indicator = tester.getRect(find.byType(StepProgress));
          for (final marker in find.byType(StepNumber).evaluate()) {
            final rect = tester.getRect(find.byWidget(marker.widget));
            expect(rect.left, greaterThanOrEqualTo(indicator.left));
            expect(rect.right, lessThanOrEqualTo(indicator.right));
          }
          expect(tester.takeException(), isNull);
        } finally {
          semantics.dispose();
        }
      },
    );
  }
}
