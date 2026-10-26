import 'dart:async';

import 'package:copypaste_flutter/app/theme/app_overlays.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart' show RenderParagraph;
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    final font = FontLoader('packages/shadcn_flutter/GeistSans');
    for (final face in ['Regular', 'Medium', 'SemiBold']) {
      font.addFont(
        rootBundle.load('packages/shadcn_flutter/lib/fonts/Geist-$face.otf'),
      );
    }
    await font.load();
  });

  testWidgets(
    'short dialog actions never wrap or truncate at compact widths and large text',
    (tester) async {
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final fixtureTheme = AppTheme.light.copyWith(
        typography: () => AppTheme.light.typography.copyWith(
          sans: () => const TextStyle(
            fontFamily: 'GeistSans',
            package: 'shadcn_flutter',
          ),
        ),
      );
      for (final width in [320.0, 390.0, 1200.0]) {
        await tester.binding.setSurfaceSize(Size(width, 800));
        for (final scale in [1.0, 2.0]) {
          for (final action in [
            'Delete',
            'Save',
            'Unpair',
            'Revoke',
            'Restore',
            'Remove',
            'Clear',
            'Exclude',
          ]) {
            await _openConfirmation(
              tester,
              mode: ThemeMode.light,
              scale: scale,
              action: action,
              fixtureTheme: fixtureTheme,
            );
            for (final label in ['Cancel', action]) {
              final button = find.widgetWithText(Button, label);
              final text = find.descendant(
                of: button,
                matching: find.byType(RichText),
              );
              expect(tester.widget<RichText>(text).maxLines, 1);
              expect(tester.widget<RichText>(text).softWrap, isFalse);
              expect(
                tester.renderObject<RenderParagraph>(text).didExceedMaxLines,
                isFalse,
                reason:
                    '$label must fit at $width pixels and $scale text scale.',
              );
            }
            await tester.tap(find.text('Cancel'));
            await tester.pumpAndSettle();
            expect(tester.takeException(), isNull);
          }
        }
      }
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

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
    'confirmations share the standard width and center the header icon',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1200, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      for (final mode in [ThemeMode.light, ThemeMode.dark]) {
        await _openConfirmation(tester, mode: mode);
        final dialog = tester.getRect(find.byType(AlertDialog));
        final icon = tester.getRect(find.byIcon(LucideIcons.trash2));
        final title = tester.getRect(find.text('Delete this clip?'));
        expect(icon.center.dy, closeTo(title.center.dy, 0.01));
        expect(dialog.width, AppOverlaySize.dialogMaxWidth);
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
            action: 'Restore',
          );
          final dialog = tester.getRect(find.byType(AlertDialog));
          final icon = tester.getRect(find.byIcon(LucideIcons.trash2));
          final titleRect = tester.getRect(find.text(title));
          expect(icon.center.dy, closeTo(titleRect.center.dy, 0.01));
          expect(dialog.left, greaterThanOrEqualTo(0));
          expect(dialog.right, lessThanOrEqualTo(320));
          for (final label in ['Cancel', 'Restore']) {
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

  testWidgets(
    'dialogs use the Android bottom safe area and follow the keyboard',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(360, 800);
      tester.view.viewPadding = const FakeViewPadding(top: 24, bottom: 24);
      tester.view.padding = const FakeViewPadding(top: 24, bottom: 24);
      addTearDown(tester.view.reset);
      await _openConfirmation(tester, mode: ThemeMode.light);
      var rect = tester.getRect(find.byType(AlertDialog));
      expect(rect.width, 360 - AppSpacing.xl * 2);
      expect(rect.bottom, closeTo(800 - 24 - AppSpacing.xl, 1));
      tester.view.viewInsets = const FakeViewPadding(bottom: 300);
      tester.view.padding = const FakeViewPadding(top: 24);
      await tester.pumpAndSettle();
      rect = tester.getRect(find.byType(AlertDialog));
      expect(rect.bottom, closeTo(800 - 300 - AppSpacing.xl, 1));
      expect(tester.takeException(), isNull);
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
    },
    variant: TargetPlatformVariant.only(TargetPlatform.android),
  );

  testWidgets(
    'desktop dialogs keep the standard centered placement',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1200, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await _openConfirmation(tester, mode: ThemeMode.light);
      final rect = tester.getRect(find.byType(AlertDialog));
      expect(rect.center, const Offset(600, 400));
      expect(rect.width, AppOverlaySize.dialogMaxWidth);
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
    },
    variant: TargetPlatformVariant({
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets(
    'actions cannot repeat or dismiss while pending and can retry a failure',
    (tester) async {
      var calls = 0;
      var pending = Completer<bool?>();
      await _openActionDialog(
        tester,
        action: () {
          calls++;
          return pending.future;
        },
      );
      await tester.tap(find.text('Confirm'));
      await tester.tap(find.text('Confirm'));
      await tester.pump();
      expect(calls, 1);
      for (final label in ['Cancel', 'Confirm']) {
        expect(
          tester.widget<Button>(find.widgetWithText(Button, label)).onPressed,
          isNull,
        );
      }
      await tester.tapAt(const Offset(1, 1));
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.binding.handlePopRoute();
      await tester.pump();
      expect(find.byType(AlertDialog), findsOneWidget);
      pending.complete(false);
      await tester.pumpAndSettle();
      expect(find.text('Action failed'), findsOneWidget);
      expect(find.text('Please try again.'), findsOneWidget);
      pending = Completer<bool?>();
      await tester.tap(find.text('Confirm'));
      await tester.pump();
      expect(find.text('Action failed'), findsNothing);
      expect(calls, 2);
      pending.complete(true);
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsNothing);
      expect(tester.takeException(), isNull);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  testWidgets('nested cancellation keeps the dialog idle without a failure', (
    tester,
  ) async {
    await _openActionDialog(tester, action: () async => null);
    await tester.tap(find.text('Confirm'));
    await tester.pumpAndSettle();
    expect(find.byType(AlertDialog), findsOneWidget);
    expect(find.text('Action failed'), findsNothing);
    expect(
      tester.widget<Button>(find.widgetWithText(Button, 'Confirm')).onPressed,
      isNotNull,
    );
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(find.byType(AlertDialog), findsNothing);
  });

  testWidgets('form validation and failed actions preserve the entered value', (
    tester,
  ) async {
    final input = TextEditingController();
    addTearDown(input.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Builder(
          builder: (context) => Button.primary(
            onPressed: () => AppOverlays.showActionDialog(
              context,
              icon: LucideIcons.pencil,
              title: const Text('Rename this device'),
              actionLabel: 'Save',
              destructive: false,
              changes: input,
              canConfirm: () => input.text.trim().isNotEmpty,
              content: TextField(controller: input),
              onConfirm: () async => false,
              failureMessage: () => 'The name could not be saved.',
            ),
            child: const Text('Open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    expect(
      tester.widget<Button>(find.widgetWithText(Button, 'Save')).onPressed,
      isNull,
    );
    await tester.enterText(find.byType(TextField), 'My device');
    await tester.pump();
    expect(
      tester.widget<Button>(find.widgetWithText(Button, 'Save')).onPressed,
      isNotNull,
    );
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(find.text('Action failed'), findsOneWidget);
    expect(input.text, 'My device');
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });

  testWidgets('unexpected action failures remain in the shared error surface', (
    tester,
  ) async {
    await _openActionDialog(
      tester,
      action: () async => throw StateError('unavailable'),
    );
    await tester.tap(find.text('Confirm'));
    await tester.pumpAndSettle();
    expect(find.text('Action failed'), findsOneWidget);
    expect(find.text('Please try again.'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'long dialog content scrolls above equal reachable footer actions',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(320, 480));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await _openActionDialog(
        tester,
        scale: 2,
        action: () async => true,
        content: Text(List.filled(40, 'Long confirmation details.').join('\n')),
      );
      final cancel = tester.getRect(find.widgetWithText(Button, 'Cancel'));
      final confirm = tester.getRect(find.widgetWithText(Button, 'Confirm'));
      expect(cancel.width, confirm.width);
      expect(cancel.height, confirm.height);
      expect(cancel.height, greaterThanOrEqualTo(AppControlSize.touch));
      expect(confirm.bottom, lessThanOrEqualTo(480 - AppSpacing.lg));
      expect(tester.takeException(), isNull);
      await tester.drag(
        find.byType(SingleChildScrollView).first,
        const Offset(0, -200),
      );
      await tester.pumpAndSettle();
      expect(
        tester
            .state<ScrollableState>(find.byType(Scrollable).first)
            .position
            .pixels,
        greaterThan(0),
      );
      expect(tester.getRect(find.widgetWithText(Button, 'Confirm')), confirm);
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
    },
    variant: TargetPlatformVariant.only(TargetPlatform.android),
  );
}

Future<void> _openActionDialog(
  WidgetTester tester, {
  required Future<bool?> Function() action,
  Widget? content,
  double scale = 1,
}) async {
  await tester.pumpWidget(
    ShadcnApp(
      theme: AppTheme.light,
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(scale)),
        child: Builder(builder: (context) => AppTheme.builder(context, child)),
      ),
      home: Builder(
        builder: (context) => Button.primary(
          onPressed: () => AppOverlays.showActionDialog(
            context,
            icon: LucideIcons.trash2,
            title: const Text('Confirm action?'),
            actionLabel: 'Confirm',
            content: content,
            onConfirm: action,
            failureMessage: () => 'Please try again.',
          ),
          child: const Text('Open'),
        ),
      ),
    ),
  );
  await tester.tap(find.text('Open'));
  await tester.pumpAndSettle();
}

Future<void> _openConfirmation(
  WidgetTester tester, {
  required ThemeMode mode,
  double scale = 1,
  String title = 'Delete this clip?',
  Widget? content,
  String action = 'Delete',
  ThemeData? fixtureTheme,
}) async {
  await tester.pumpWidget(
    ShadcnApp(
      theme: fixtureTheme ?? AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: mode,
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(scale)),
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
