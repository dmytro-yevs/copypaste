import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/ai_tools/models/ai_tools_models.dart';
import 'package:copypaste_flutter/features/ai_tools/view/ai_approval_dialog.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  final l10n = lookupAppLocalizations(AppLocalization.fallback);

  testWidgets('Deny holds the initial focus and Escape denies', (tester) async {
    final results = <AiApprovalDecision?>[];
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: Builder(
            builder: (context) => Button.primary(
              key: const ValueKey('open'),
              onPressed: () async => results.add(
                await showAiApprovalDialog(
                  context,
                  request: AiApprovalRequest(
                    requestId: 'request-1',
                    kind: AiApprovalKind.access,
                    clientName: 'Cursor',
                    executable: '/usr/local/bin/cursor',
                    expiresAt: DateTime(2026, 11, 13),
                  ),
                  defaultLookback: AiToolLookback.week,
                ),
              ),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(const ValueKey('open')));
    await tester.pumpAndSettle();

    expect(find.text(l10n.aiToolsApprovalTitle('Cursor')), findsOneWidget);
    final deny = tester.widget<Button>(
      find.byKey(const ValueKey('ai-decision-deny')),
    );
    expect(deny.focusNode!.hasFocus, isTrue);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    expect(results, hasLength(1));
    expect(results.single?.allowed, isFalse);
    expect(find.text(l10n.aiToolsApprovalTitle('Cursor')), findsNothing);
  });
}
