import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/devices/device_presentation.dart';
import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/shared/step_progress.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('a declined pairing names the step it ended at', (tester) async {
    AppStrings.useLocale(const Locale('de'));
    addTearDown(() => AppStrings.useLocale(AppLocalization.fallback));
    final semantics = tester.ensureSemantics();
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.dark,
        builder: AppTheme.builder,
        home: Scaffold(
          child: DevicePresentation.pairingStepProgress(
            PairingProgress.resolve(
              mode: PairingEntryMode.enterCode,
              state: PairingState.rejected,
              furthestState: PairingState.awaitingConfirmation,
            ),
          ),
        ),
      ),
    );
    // Two steps happened, the third failed, the last was never reached.
    expect(find.byIcon(LucideIcons.check), findsNWidgets(2));
    expect(find.byIcon(LucideIcons.x), findsOneWidget);
    // This device did not reject, so the outcome names no party.
    expect(
      tester.getSemantics(find.byType(StepProgress)),
      isSemantics(
        label: 'Schritt 3 von 4: Bestätigung',
        value: 'Kopplung nicht abgeschlossen',
      ),
    );
    semantics.dispose();
  });
}
