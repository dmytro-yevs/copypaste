import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('leading artwork stays square beside enlarged button text', (
    tester,
  ) async {
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: (context, child) => AppTheme.builder(
          context,
          MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: const TextScaler.linear(2)),
            child: child!,
          ),
        ),
        home: Center(
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Button.secondary(
                onPressed: () {},
                leading: const AppIconFrame(
                  size: AppIconSize.sm,
                  child: CircularProgressIndicator(
                    value: 0.25,
                    animated: false,
                  ),
                ),
                child: const Text('25%'),
              ),
              Button.primary(
                onPressed: () {},
                leading: const AppIconFrame(child: Icon(LucideIcons.copy)),
                child: const Text('Copy'),
              ),
            ],
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    for (final artwork in [
      find.byType(CircularProgressIndicator),
      find.byIcon(LucideIcons.copy),
    ]) {
      final size = tester.getSize(artwork);
      expect(size, const Size.square(AppIconSize.sm));
      expect(
        find.ancestor(of: artwork, matching: find.byType(AspectRatio)),
        findsOneWidget,
      );
    }
    expect(tester.takeException(), isNull);
  });

  testWidgets('tight rectangular parents preserve icon size and proportions', (
    tester,
  ) async {
    await tester.pumpWidget(
      const ShadcnApp(
        home: Center(
          child: SizedBox(
            width: AppControlSize.touch,
            height: AppControlSize.regular,
            child: AppIconFrame(
              size: AppIconSize.sm,
              child: Icon(LucideIcons.download),
            ),
          ),
        ),
      ),
    );
    expect(
      tester.getSize(find.byIcon(LucideIcons.download)),
      const Size.square(AppIconSize.sm),
    );
    expect(tester.takeException(), isNull);
  });
}
