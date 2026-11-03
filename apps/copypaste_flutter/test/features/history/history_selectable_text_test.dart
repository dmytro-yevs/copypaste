import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/history/view/history_selectable_text.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/rendering.dart' show RenderParagraph;
import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

String _largeClip() {
  final buffer = StringBuffer();
  for (var i = 0; i < 20; i++) {
    buffer.writeln('header $i ${'word ' * (i % 7)}');
  }
  buffer.writeln(List.generate(1500, (i) => 'token$i').join(' '));
  for (var i = 0; buffer.length < 48 * 1024; i++) {
    buffer.writeln('line $i ${'word ' * (i % 17)}');
    if (i % 40 == 0) buffer.writeln();
  }
  return buffer.toString();
}

Future<void> _pumpInScrollView(WidgetTester tester, Widget child) async {
  tester.view.physicalSize = const Size(600, 3000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    ShadcnApp(
      theme: AppTheme.light,
      home: Scaffold(
        child: SingleChildScrollView(
          child: Align(alignment: Alignment.topLeft, child: child),
        ),
      ),
    ),
  );
  await tester.pump();
}

Future<void> _drag(WidgetTester tester, Offset from, Offset to) async {
  final gesture = await tester.startGesture(
    from,
    kind: PointerDeviceKind.mouse,
  );
  await tester.pump();
  await gesture.moveTo(Offset.lerp(from, to, 0.5)!);
  await tester.pump();
  await gesture.moveTo(to);
  await tester.pump();
  await gesture.up();
  await tester.pump();
}

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

  testWidgets(
    'a large clip lays out nearby slices that wrap like one paragraph',
    (tester) async {
      final text = _largeClip();
      await _pumpInScrollView(
        tester,
        HistorySelectableText(text, onCopy: (_) async => true),
      );
      final origin = tester.getTopLeft(find.byType(HistorySelectableText));
      final height = tester.getSize(find.byType(HistorySelectableText)).height;
      final paragraphs = tester.renderObjectList<RenderParagraph>(
        find.descendant(
          of: find.byType(HistorySelectableText),
          matching: find.byType(RichText),
        ),
      );
      final slices = <(int, double)>[];
      var cursor = 0;
      var laidOut = 0;
      for (final paragraph in paragraphs) {
        final slice = paragraph.text.toPlainText();
        final start = text.indexOf(slice, cursor);
        expect(start, isNonNegative);
        cursor = start + slice.length;
        laidOut += slice.length;
        if (slice.isEmpty || slice.startsWith('\n')) continue;
        final box = paragraph.getBoxesForSelection(
          const TextSelection(baseOffset: 0, extentOffset: 1),
        );
        slices.add((
          start,
          box.first.top + paragraph.localToGlobal(Offset.zero).dy - origin.dy,
        ));
      }
      final longLine = text.indexOf('token0');
      final longLineEnd = text.indexOf('\n', longLine);
      expect(
        slices.where((s) => s.$1 > longLine && s.$1 < longLineEnd),
        isNotEmpty,
      );
      expect(laidOut, lessThan(text.length ~/ 2));

      await _pumpInScrollView(tester, SelectableText(text));
      final editable = tester
          .state<EditableTextState>(find.byType(EditableText))
          .renderEditable;
      expect(
        tester.getSize(find.byType(SelectableText)).height,
        moreOrLessEquals(height, epsilon: 0.5),
      );
      for (final (start, top) in slices) {
        expect(
          editable.getLineAtOffset(TextPosition(offset: start)).start,
          start,
        );
        final box = editable.getBoxesForSelection(
          TextSelection(baseOffset: start, extentOffset: start + 1),
        );
        expect(box.first.top, moreOrLessEquals(top, epsilon: 0.5));
      }
    },
  );

  testWidgets(
    'select-all and drag selection on a large clip copy what one paragraph would',
    (tester) async {
      final text = _largeClip();
      final copied = <String>[];
      await _pumpInScrollView(
        tester,
        HistorySelectableText(
          text,
          onCopy: (value) async {
            copied.add(value);
            return true;
          },
        ),
      );
      final origin = tester.getTopLeft(find.byType(HistorySelectableText));
      const end = Offset(300, 1500);
      await _drag(tester, origin + const Offset(1, 1), origin + end);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
      await tester.pump();
      expect(copied.length, 2);
      expect(copied.last, text);

      var selection = const TextSelection.collapsed(offset: 0);
      await _pumpInScrollView(
        tester,
        SelectableText(
          text,
          onSelectionChanged: (value, _) => selection = value,
        ),
      );
      await _drag(tester, origin + const Offset(1, 1), origin + end);
      expect(copied.first, contains('token0'));
      expect(copied.first, selection.textInside(text));
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );
}
