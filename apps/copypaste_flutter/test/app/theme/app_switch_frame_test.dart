import 'package:copypaste_flutter/app/theme/app_switch_frame.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

Widget _app(Widget home, {Locale? locale, double textScale = 1}) => ShadcnApp(
  theme: AppTheme.light,
  locale: locale,
  localizationsDelegates: AppLocalization.delegates,
  supportedLocales: AppLocalization.supportedLocales,
  builder: (context, child) => MediaQuery(
    data: MediaQuery.of(
      context,
    ).copyWith(textScaler: TextScaler.linear(textScale)),
    child: Builder(builder: (context) => AppTheme.builder(context, child)),
  ),
  home: Scaffold(child: Center(child: home)),
);

BorderRadiusGeometry? _radius(WidgetTester tester, Finder box) =>
    ((tester.widget<Container>(box).decoration!) as BoxDecoration).borderRadius;

final _track = find.descendant(
  of: find.byType(Switch),
  matching: find.byType(AnimatedContainer),
);

final _marks = find.descendant(
  of: find.byType(AppSwitchFrame),
  matching: find.byType(CustomPaint),
);

/// The mark drawn over the track: its method, its centre and its colour.
(Symbol, Offset, Color)? _mark(WidgetTester tester) {
  (Symbol, Offset, Color)? mark;
  expect(
    tester.renderObject(_marks),
    paints..everything((method, arguments) {
      if (method == #drawRect) {
        final paint = arguments[1] as Paint;
        mark = (method, (arguments[0] as Rect).center, paint.color);
      } else if (method == #drawCircle) {
        final paint = arguments[2] as Paint;
        mark = (method, arguments[0] as Offset, paint.color);
      }
      return true;
    }),
  );
  return mark;
}

double _contrast(Color first, Color second) {
  final lightest = [first.computeLuminance(), second.computeLuminance()]
    ..sort();
  return (lightest.last + 0.05) / (lightest.first + 0.05);
}

void main() {
  testWidgets(
    'the shared theme shapes the track, thumb and focus ring',
    (tester) async {
      await tester.pumpWidget(
        _app(AppSwitchFrame(child: Switch(value: true, onChanged: (_) {}))),
      );
      await tester.pumpAndSettle();

      final track = find.descendant(
        of: find.byType(Switch),
        matching: find.byType(AnimatedContainer),
      );
      final decoration =
          tester.widget<AnimatedContainer>(track).decoration! as BoxDecoration;
      expect(decoration.borderRadius, BorderRadius.circular(AppRadius.xs));
      expect(decoration.color, AppTheme.light.colorScheme.foreground);
      final thumb = find.descendant(
        of: track,
        matching: find.byWidgetPredicate(
          (widget) => widget is Container && widget.decoration is BoxDecoration,
        ),
      );
      expect(
        _radius(tester, thumb.last),
        BorderRadius.circular(AppRadius.xs / 2),
      );

      // The library enlarges every control on touch platforms by itself.
      final touch = defaultTargetPlatform == TargetPlatform.android;
      expect(
        tester.getSize(track),
        touch ? const Size(45, 25) : const Size(36, 20),
      );

      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pumpAndSettle();
      final ring = tester.widget<DecoratedBox>(
        find.descendant(
          of: find.byType(FocusOutline),
          matching: find.byWidgetPredicate(
            (widget) =>
                widget is DecoratedBox && widget.decoration is ShapeDecoration,
          ),
        ),
      );
      final side =
          ((ring.decoration as ShapeDecoration).shape as RoundedRectangleBorder)
              .side;
      expect(side.width, 3);
      expect(side.color.a, greaterThan(0));
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
    }),
  );

  testWidgets('the state word follows the value and stays out of semantics', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    var value = false;
    await tester.pumpWidget(
      _app(
        StatefulBuilder(
          builder: (context, setState) => Semantics(
            label: 'Start at login',
            toggled: value,
            child: AppSwitchFrame(
              child: Switch(
                value: value,
                onChanged: (next) => setState(() => value = next),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Off').hitTestable(), findsOneWidget);
    expect(find.text('On').hitTestable(), findsNothing);
    final left = tester.getTopLeft(find.byType(Switch)).dx;

    await tester.tap(find.byType(Switch));
    await tester.pumpAndSettle();
    expect(find.text('On').hitTestable(), findsOneWidget);
    expect(find.text('Off').hitTestable(), findsNothing);
    expect(tester.getTopLeft(find.byType(Switch)).dx, left);

    expect(
      tester.getSemantics(find.bySemanticsLabel('Start at login')),
      isSemantics(label: 'Start at login', isToggled: true),
    );
    expect(find.bySemanticsLabel(RegExp(r'\bOn\b')), findsNothing);
    semantics.dispose();
  });

  testWidgets('long state words never overflow a narrow row', (tester) async {
    Widget row({required bool stacked, required double textScale}) => _app(
      SizedBox(
        width: 120,
        child: Row(
          children: [
            const Expanded(child: Text('Synchronisierung')),
            Flexible(
              child: AppSwitchFrame(
                stacked: stacked,
                child: Switch(value: false, onChanged: (_) {}),
              ),
            ),
          ],
        ),
      ),
      locale: const Locale('es'),
      textScale: textScale,
    );

    await tester.pumpWidget(row(stacked: false, textScale: 2));
    await tester.pumpAndSettle();
    expect(find.text('Desactivado'), findsOneWidget);
    expect(tester.takeException(), isNull);

    await tester.pumpWidget(row(stacked: true, textScale: 1));
    await tester.pumpAndSettle();
    final word = tester.getRect(find.text('Desactivado'));
    final control = tester.getRect(find.byType(Switch));
    expect(word.top, greaterThan(control.bottom));
    expect(word.right, control.right);
    expect(tester.takeException(), isNull);
  });

  for (final rtl in [false, true]) {
    testWidgets(
      'marks sit in the gap the thumb leaves (rtl: $rtl)',
      (tester) async {
        Future<void> pump({required bool value, bool enabled = true}) async {
          await tester.pumpWidget(
            _app(
              Directionality(
                textDirection: rtl ? TextDirection.rtl : TextDirection.ltr,
                child: AppSwitchFrame(
                  showState: false,
                  child: Switch(
                    value: value,
                    enabled: enabled,
                    onChanged: enabled ? (_) {} : null,
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();
        }

        final colors = AppTheme.light.colorScheme;
        for (final value in [true, false]) {
          await pump(value: value);
          final track = tester.getRect(_track);
          final thumb = tester.getRect(
            find.descendant(
              of: find.byType(AnimatedPositionedDirectional),
              matching: find.byType(Container),
            ),
          );
          final inset = (track.height - thumb.height) / 2;
          final gap = thumb.left - track.left > track.right - thumb.right
              ? Rect.fromLTRB(
                  track.left + inset,
                  thumb.top,
                  thumb.left,
                  thumb.bottom,
                )
              : Rect.fromLTRB(
                  thumb.right,
                  thumb.top,
                  track.right - inset,
                  thumb.bottom,
                );
          final (method, center, color) = _mark(tester)!;
          expect(method, value ? #drawRect : #drawCircle);
          expect(
            color,
            isSameColorAs(
              value ? colors.primaryForeground : colors.mutedForeground,
            ),
          );
          final expected = gap.center - track.topLeft;
          expect(center.dx, moreOrLessEquals(expected.dx));
          expect(center.dy, moreOrLessEquals(expected.dy));
          // On sits at the start of the track and off at its end.
          final atStart = (center.dx < track.width / 2) != rtl;
          expect(atStart, value);
        }
        // Only the off track carries the solid outline.
        expect(tester.renderObject(_marks), paints..rrect());

        await pump(value: true, enabled: false);
        expect(_mark(tester), isNull);
        expect(
          tester.renderObject(_marks),
          paints
            ..path()
            ..path()
            ..path(),
        );
        final fill = tester.widget<AnimatedContainer>(_track).decoration!;
        expect((fill as BoxDecoration).color, Colors.transparent);
      },
      variant: TargetPlatformVariant({
        TargetPlatform.android,
        TargetPlatform.macOS,
      }),
    );
  }

  test('marks and the off outline keep 3:1 in both appearances', () {
    for (final colors in [
      AppTheme.light.colorScheme,
      AppTheme.dark.colorScheme,
    ]) {
      expect(
        _contrast(colors.primaryForeground, colors.primary),
        greaterThanOrEqualTo(3),
      );
      for (final surface in [
        colors.background,
        colors.card,
        colors.popover,
        colors.secondary,
        colors.muted,
        colors.accent,
      ]) {
        final well = Color.alphaBlend(
          colors.foreground.withValues(alpha: 0.18),
          surface,
        );
        final outline = Color.alphaBlend(
          colors.foreground.withValues(alpha: 0.5),
          surface,
        );
        expect(
          _contrast(colors.mutedForeground, well),
          greaterThanOrEqualTo(3),
        );
        expect(_contrast(outline, surface), greaterThanOrEqualTo(3));
      }
    }
  });

  testWidgets(
    'the touch target toggles from its whole 48 px area',
    (tester) async {
      var value = false;
      var enabled = true;
      late StateSetter rebuild;
      await tester.pumpWidget(
        _app(
          StatefulBuilder(
            builder: (context, setState) {
              rebuild = setState;
              return AppSwitchFrame(
                showState: false,
                child: Switch(
                  value: value,
                  enabled: enabled,
                  onChanged: enabled
                      ? (next) => setState(() => value = next)
                      : null,
                ),
              );
            },
          ),
        ),
      );
      await tester.pumpAndSettle();
      final touch = defaultTargetPlatform == TargetPlatform.android;
      final frame = tester.getSize(find.byType(AppSwitchFrame));
      final track = tester.getRect(_track);
      expect(
        frame,
        touch
            ? const Size(AppControlSize.touch, AppControlSize.touch)
            : track.size,
      );

      // Above the track, inside the touch target.
      await tester.tapAt(track.topCenter - const Offset(0, 8));
      await tester.pumpAndSettle();
      expect(value, touch);
      expect(tester.getRect(_track), track);

      if (!touch) return;
      rebuild(() => enabled = false);
      await tester.pumpAndSettle();
      await tester.tapAt(track.topCenter - const Offset(0, 8));
      await tester.pumpAndSettle();
      expect(value, isTrue);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
    }),
  );

  testWidgets('a pending switch reads as not interactive', (tester) async {
    final semantics = tester.ensureSemantics();
    await tester.pumpWidget(
      _app(
        const AppSwitchFrame(
          child: Switch(value: true, enabled: false, onChanged: null),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(
      tester.getSemantics(find.byType(Switch)),
      isSemantics(
        hasToggledState: true,
        isToggled: true,
        hasEnabledState: true,
        isEnabled: false,
      ),
    );
    semantics.dispose();
  });
}
