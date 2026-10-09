import 'package:copypaste_flutter/app/theme/app_overlays.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets(
    'drawers use the right edge from 800 pixels and bottom edge below it',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      for (final width in [799.0, 800.0, 1200.0]) {
        tester.view.physicalSize = Size(width, 800);
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            builder: AppTheme.builder,
            home: Scaffold(
              child: Builder(
                builder: (context) => Button.primary(
                  onPressed: () => showOverlay<void>(
                    context,
                    AppOverlays.drawerConfiguration(context),
                    builder: (context) => ConstrainedBox(
                      key: const ValueKey('adaptive-drawer'),
                      constraints: AppOverlays.drawerContentConstraints(
                        context,
                      ),
                      child: Scaffold(
                        child: Button.ghost(
                          onPressed: () => closeDrawer(context),
                          child: const Text('Close'),
                        ),
                      ),
                    ),
                  ),
                  child: const Text('Open'),
                ),
              ),
            ),
          ),
        );
        await tester.tap(find.text('Open'));
        await tester.pumpAndSettle();
        final wrapper = tester.widget<DrawerWrapper>(
          find.byType(DrawerWrapper),
        );
        final rect = tester.getRect(
          find.byKey(const ValueKey('adaptive-drawer')),
        );
        if (width >= 800) {
          expect(wrapper.position, OverlayPosition.right);
          expect(wrapper.showDragHandle, isFalse);
          expect(rect.width, closeTo(AppOverlaySize.drawerPanelWidth, 2));
          expect(rect.height, closeTo(800, 2));
          expect(rect.right, closeTo(width, 2));
          expect(rect.top, closeTo(0, 2));
        } else {
          expect(wrapper.position, OverlayPosition.bottom);
          expect(rect.width, closeTo(width, 2));
          expect(
            rect.height,
            closeTo(800 * AppOverlaySize.drawerHeightFactor, 2),
          );
          expect(
            tester.getRect(find.byType(DrawerWrapper)).bottom,
            closeTo(800, 2),
          );
        }
        expect(tester.takeException(), isNull);
        await tester.tap(find.text('Close'));
        await tester.pumpAndSettle();
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.android,
    }),
  );

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
    final leading = tester.widget<Icon>(find.byIcon(LucideIcons.pencil));
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

  testWidgets(
    'confirmation fits its content and centers the header icon',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1200, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      for (final mode in [ThemeMode.light, ThemeMode.dark]) {
        await _openConfirmation(tester, mode: mode);
        final dialog = tester.getRect(find.byType(AlertDialog));
        final icon = tester.getRect(find.byIcon(LucideIcons.trash2));
        final title = tester.getRect(find.text('Delete this clip?'));
        expect(icon.center.dy, closeTo(title.center.dy, 0.01));
        expect(dialog.width, lessThan(AppOverlaySize.dialogMaxWidth));
        expect(icon.left - dialog.left, lessThanOrEqualTo(AppSpacing.xl + 2));
        expect(tester.takeException(), isNull);
        await tester.tap(find.text('Cancel'));
        await tester.pumpAndSettle();
        expect(find.byType(AlertDialog), findsNothing);
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'long headers and actions fit a 320px viewport at 200 percent text',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(320, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      for (final mode in [ThemeMode.light, ThemeMode.dark]) {
        for (final title in [
          'Delete this clip?',
          'Replace local history?',
          'Revoke a device with a long name?',
          'Remove a module with a long name?',
        ]) {
          await _openConfirmation(
            tester,
            mode: mode,
            scale: 2,
            title: title,
            content: const Text('This action changes the local data.'),
            action: 'Choose backup',
          );
          final dialog = tester.getRect(find.byType(AlertDialog));
          final icon = tester.getRect(find.byIcon(LucideIcons.trash2));
          final titleRect = tester.getRect(find.text(title));
          expect(icon.center.dy, closeTo(titleRect.center.dy, 0.01));
          expect(dialog.left, greaterThanOrEqualTo(0));
          expect(dialog.right, lessThanOrEqualTo(320));
          for (final label in ['Cancel', 'Choose backup']) {
            final actionRect = tester.getRect(find.text(label));
            expect(dialog.contains(actionRect.topLeft), isTrue);
            expect(dialog.contains(actionRect.bottomRight), isTrue);
          }
          expect(tester.takeException(), isNull);
          await tester.tap(find.text('Cancel'));
          await tester.pumpAndSettle();
        }
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

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

Future<void> _openConfirmation(
  WidgetTester tester, {
  required ThemeMode mode,
  double scale = 1,
  String title = 'Delete this clip?',
  Widget? content,
  String action = 'Delete',
}) async {
  await tester.pumpWidget(
    ShadcnApp(
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: mode,
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(
          context,
        ).copyWith(textScaler: TextScaler.linear(scale)),
        child: Builder(builder: (context) => AppTheme.builder(context, child)),
      ),
      home: Builder(
        builder: (context) => Button.primary(
          onPressed: () => AppOverlays.showDialog<void>(
            context,
            builder: (context) => AppOverlays.alertDialog(
              icon: LucideIcons.trash2,
              title: Text(title),
              content: content,
              actions: [
                Button.ghost(
                  onPressed: () => Navigator.pop(context),
                  child: const Text('Cancel'),
                ),
                Button.destructive(
                  onPressed: () => Navigator.pop(context),
                  child: Text(action),
                ),
              ],
            ),
          ),
          child: const Text('Open'),
        ),
      ),
    ),
  );
  await tester.tap(find.text('Open'));
  await tester.pumpAndSettle();
}
