import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

/// Whether keyboard focus currently sits inside the subtree of [finder].
bool focusInside(WidgetTester tester, Finder finder) {
  final focused = FocusManager.instance.primaryFocus?.context;
  if (focused == null) return false;
  final target = tester.element(finder);
  if (identical(focused, target)) return true;
  var inside = false;
  focused.visitAncestorElements((element) {
    inside = identical(element, target);
    return !inside;
  });
  return inside;
}

/// Focuses [field], presses Tab and checks that focus left the field.
Future<void> expectTabLeaves(WidgetTester tester, Finder field) async {
  await tester.tap(field);
  await tester.pump();
  expect(focusInside(tester, field), isTrue, reason: 'field takes focus');
  await tester.sendKeyEvent(LogicalKeyboardKey.tab);
  await tester.pump();
  expect(focusInside(tester, field), isFalse, reason: 'Tab leaves the field');
  expect(FocusManager.instance.primaryFocus?.context, isNotNull);
}
