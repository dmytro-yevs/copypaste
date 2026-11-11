import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

Widget _app(Widget home) => ShadcnApp(
  theme: AppTheme.light,
  builder: AppTheme.builder,
  home: Scaffold(child: Center(child: home)),
);

/// The ring side painted around [control], or null while it has none.
BorderSide? _ring(WidgetTester tester, Finder control) {
  final outline = find.descendant(
    of: control,
    matching: find.byType(FocusOutline),
  );
  if (!tester.widget<FocusOutline>(outline.first).focused) return null;
  final box = tester.widget<DecoratedBox>(
    find.descendant(
      of: outline.first,
      matching: find.byWidgetPredicate(
        (widget) =>
            widget is DecoratedBox && widget.decoration is ShapeDecoration,
      ),
    ),
  );
  return ((box.decoration as ShapeDecoration).shape as RoundedRectangleBorder)
      .side;
}

void main() {
  testWidgets('a ring follows keyboard focus only and never moves layout', (
    tester,
  ) async {
    const first = ValueKey('first');
    const second = ValueKey('second');
    await tester.pumpWidget(
      _app(
        Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Button.secondary(
              key: first,
              onPressed: () {},
              child: const Text('First'),
            ),
            Button.secondary(
              key: second,
              onPressed: () {},
              child: const Text('Second'),
            ),
          ],
        ),
      ),
    );
    await tester.pumpAndSettle();
    final firstRect = tester.getRect(find.byKey(first));
    final secondRect = tester.getRect(find.byKey(second));

    await tester.tap(find.byKey(first));
    await tester.pumpAndSettle();
    expect(_ring(tester, find.byKey(first)), isNull);
    expect(_ring(tester, find.byKey(second)), isNull);

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    final focused = [
      first,
      second,
    ].where((key) => _ring(tester, find.byKey(key)) != null).toList();
    expect(focused, hasLength(1));
    final ring = _ring(tester, find.byKey(focused.single))!;
    expect(ring.color.a, greaterThan(0));
    expect(ring.width, greaterThan(0));
    expect(tester.getRect(find.byKey(first)), firstRect);
    expect(tester.getRect(find.byKey(second)), secondRect);

    await tester.tap(find.byKey(focused.single));
    await tester.pumpAndSettle();
    expect(_ring(tester, find.byKey(focused.single)), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('a search field that leaves the cue to its list has no ring', (
    tester,
  ) async {
    const plain = ValueKey('plain');
    const search = ValueKey('search');
    final plainFocus = FocusNode();
    final searchFocus = FocusNode();
    addTearDown(plainFocus.dispose);
    addTearDown(searchFocus.dispose);
    await tester.pumpWidget(
      _app(
        SizedBox(
          width: 240,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(key: plain, focusNode: plainFocus),
              Builder(
                builder: (context) => Theme(
                  data: AppTheme.clipboardSearchTheme(context),
                  child: TextField(key: search, focusNode: searchFocus),
                ),
              ),
            ],
          ),
        ),
      ),
    );

    plainFocus.requestFocus();
    await tester.pumpAndSettle();
    expect(_ring(tester, find.byKey(plain))!.color.a, greaterThan(0));

    searchFocus.requestFocus();
    await tester.pumpAndSettle();
    expect(_ring(tester, find.byKey(search))!.color.a, 0);
  });
}
