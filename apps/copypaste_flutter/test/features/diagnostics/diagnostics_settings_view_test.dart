import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/diagnostics/controller/diagnostics_controller.dart';
import 'package:copypaste_flutter/features/diagnostics/models/diagnostic_models.dart';
import 'package:copypaste_flutter/features/diagnostics/view/diagnostics_settings_view.dart';
import 'package:copypaste_flutter/shared/state_view.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'diagnostics_test_support.dart';

void main() {
  Future<DiagnosticsController> pump(
    WidgetTester tester,
    FakeDiagnosticsRepository repository,
  ) async {
    final controller = DiagnosticsController(
      repository: repository,
      searchDelay: Duration.zero,
    );
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: SingleChildScrollView(
            child: DiagnosticsSettingsView(controller: controller),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    return controller;
  }

  testWidgets('lists events, pages and filters to an empty state', (
    tester,
  ) async {
    await pump(
      tester,
      FakeDiagnosticsRepository([
        diagnosticEntry(0, severity: DiagnosticSeverity.error),
        for (var n = 1; n < 60; n++) diagnosticEntry(n),
      ]),
    );

    expect(find.text('event 0'), findsOneWidget);
    expect(find.text('Error'), findsOneWidget);
    expect(
      tester.getTopLeft(find.text('event 0')).dx,
      tester.getTopLeft(find.text('event 1')).dx,
    );
    expect(find.text('event 55'), findsNothing);

    final pagination = find.byKey(const ValueKey('diagnostics-pagination'));
    await tester.ensureVisible(pagination);
    await tester.tap(find.descendant(of: pagination, matching: find.text('2')));
    await tester.pumpAndSettle();
    expect(find.text('event 55'), findsOneWidget);
    expect(find.text('event 0'), findsNothing);

    await tester.enterText(
      find.byKey(const ValueKey('diagnostics-search')),
      'no such event',
    );
    await tester.pumpAndSettle();
    expect(find.byType(StateView), findsOneWidget);
    expect(find.text('No log entries'), findsOneWidget);
  });
}
