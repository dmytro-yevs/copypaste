import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/shared/step_progress.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('a finished operation that is not running shows no activity', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: const Scaffold(
          child: StepProgress(
            steps: ['Connect', 'Confirm'],
            current: 2,
            running: false,
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.byIcon(LucideIcons.check), findsNWidgets(2));
    expect(find.byType(LinearProgressIndicator), findsNothing);
    expect(
      tester.getSemantics(find.byType(StepProgress)).label,
      'Step 2 of 2: Confirm',
    );
    expect(tester.takeException(), isNull);
    semantics.dispose();
  });
}
