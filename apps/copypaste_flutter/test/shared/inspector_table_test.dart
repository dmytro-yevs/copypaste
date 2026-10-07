import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/shared/inspector_table.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  for (final scale in [1.0, 2.0]) {
    testWidgets(
      'metadata stays aligned and wraps at phone width, scale $scale',
      (tester) async {
        tester.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            builder: AppTheme.builder,
            home: const Scaffold(
              child: Center(
                child: SizedBox(
                  width: 256,
                  child: InspectorTable(
                    tableKey: ValueKey('metadata'),
                    rows: [
                      (
                        label: 'Ping',
                        value: SelectableText(
                          '24 ms\nAuthenticated round trip',
                        ),
                      ),
                      (
                        label: 'LAN endpoint',
                        value: SelectableText('192.168.50.232:62951'),
                      ),
                      (
                        label: 'Last successful sync',
                        value: SelectableText('Oct 8, 2026 00:54'),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        );
        final table = find.byKey(const ValueKey('metadata'));
        final label = find.text('Ping');
        final value = find.text('24 ms\nAuthenticated round trip');
        expect(
          tester.getRect(label).center.dy,
          closeTo(tester.getRect(value).center.dy, 0.01),
        );
        final style = DefaultTextStyle.of(tester.element(value)).style;
        expect(
          style.fontSize,
          Theme.of(tester.element(value)).typography.xSmall.fontSize,
        );
        expect(style.leadingDistribution, TextLeadingDistribution.even);
        final bounds = tester.getRect(table);
        for (final value in ['192.168.50.232:62951', 'Oct 8, 2026 00:54']) {
          final rect = tester.getRect(find.text(value));
          expect(rect.left, greaterThan(bounds.left));
          expect(rect.right, lessThanOrEqualTo(bounds.right - AppSpacing.sm));
        }
        expect(tester.takeException(), isNull);
      },
      variant: TargetPlatformVariant({
        TargetPlatform.android,
        TargetPlatform.macOS,
        TargetPlatform.windows,
      }),
    );
  }
}
