import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/view/history_selectable_text.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets(
    'keyboard, toolbar and accessibility copy keep the selected text callback',
    (tester) async {
      final copied = <String>[];
      final semantics = tester.ensureSemantics();
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          home: Scaffold(
            child: Center(
              child: HistorySelectableText(
                'first selected last',
                onCopy: (text) async {
                  copied.add(text);
                  return true;
                },
              ),
            ),
          ),
        ),
      );
      final editable = tester.state<EditableTextState>(
        find.byType(EditableText),
      );
      editable.userUpdateTextEditingValue(
        const TextEditingValue(
          text: 'first selected last',
          selection: TextSelection(baseOffset: 6, extentOffset: 14),
        ),
        SelectionChangedCause.keyboard,
      );
      await tester.pump();
      Actions.invoke(editable.context, CopySelectionTextIntent.copy);
      await tester.pump();
      expect(copied, ['selected']);

      editable.showToolbar();
      await tester.pumpAndSettle();
      await tester.tap(find.text('Copy'));
      await tester.pumpAndSettle();
      expect(copied, ['selected', 'selected']);

      final node = tester.getSemantics(find.byType(HistorySelectableText));
      expect(node.getSemanticsData().hasAction(SemanticsAction.copy), isTrue);
      tester
          .renderObject(find.byType(HistorySelectableText))
          .owner!
          .semanticsOwner!
          .performAction(node.id, SemanticsAction.copy);
      await tester.pump();
      expect(copied, ['selected', 'selected', 'selected']);
      semantics.dispose();
    },
  );
}
