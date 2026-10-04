import 'package:copypaste_flutter/app/theme/app_overlays.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('uses one compact dialog surface on wide layouts', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1200, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
        builder: AppTheme.builder,
        home: Builder(
          builder: (context) => Button.primary(
            onPressed: () {
              AppOverlays.showDialog<void>(
                context,
                builder: (dialogContext) => AppOverlays.alertDialog(
                  icon: LucideIcons.pencil,
                  title: const Text('Rename this device'),
                  content: TextField(
                    placeholder: const Text('Device name'),
                    decoration: AppOverlays.dialogFieldDecoration(
                      dialogContext,
                    ),
                  ),
                  actions: [
                    Button.ghost(
                      onPressed: () => Navigator.pop(dialogContext),
                      child: const Text('Cancel'),
                    ),
                    Button.primary(
                      onPressed: () => Navigator.pop(dialogContext),
                      child: const Text('Save'),
                    ),
                  ],
                ),
              );
            },
            child: const Text('Open'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();

    final dialog = find.byType(AlertDialog);
    final dialogWidget = tester.widget<AlertDialog>(dialog);
    final leading = dialogWidget.leading! as Icon;
    final field = tester.widget<TextField>(find.byType(TextField));
    final fieldDecoration = field.decoration!;

    expect(tester.getSize(dialog).width, AppOverlaySize.dialogMaxWidth);
    expect(leading.icon, LucideIcons.pencil);
    expect(leading.size, AppIconSize.sm);
    expect(leading.size, AppTheme.light.typography.large.fontSize);
    expect(dialogWidget.padding, const EdgeInsets.all(AppSpacing.xl));
    expect(dialogWidget.barrierColor, AppOverlays.scrimColor);
    expect(fieldDecoration.color, AppTheme.light.colorScheme.accent);
    expect(
      fieldDecoration.borderRadius,
      const BorderRadius.all(Radius.circular(AppRadius.md)),
    );
  });

  testWidgets('keeps the shared dialog inside compact viewports', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(360, 640));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
        builder: AppTheme.builder,
        home: Builder(
          builder: (context) => Button.primary(
            onPressed: () {
              AppOverlays.showDialog<void>(
                context,
                builder: (dialogContext) => AppOverlays.alertDialog(
                  icon: LucideIcons.info,
                  title: const Text('CopyPaste'),
                  content: const Text('Dialog content'),
                  actions: [
                    Button.primary(
                      onPressed: () => Navigator.pop(dialogContext),
                      child: const Text('Done'),
                    ),
                  ],
                ),
              );
            },
            child: const Text('Open'),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();

    final dialogRect = tester.getRect(find.byType(AlertDialog));
    expect(dialogRect.left, greaterThanOrEqualTo(0));
    expect(dialogRect.right, lessThanOrEqualTo(360));
  });
}
