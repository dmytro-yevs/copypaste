import 'dart:io';

import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_overlays.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  test('uses one system light and dark shadcn theme', () {
    expect(AppTheme.mode, ThemeMode.system);
    expect(AppTheme.light.colorScheme.brightness, Brightness.light);
    expect(AppTheme.dark.colorScheme.brightness, Brightness.dark);
  });

  test('uses the ChatGPT application surface palette and no focus ring', () {
    final light = AppTheme.light.colorScheme;
    final dark = AppTheme.dark.colorScheme;

    expect(light.background, const Color(0xFFFFFFFF));
    expect(light.foreground, const Color(0xFF1A1C1F));
    expect(light.card, const Color(0xFFFFFFFF));
    expect(light.secondary, AppTheme.lightSidebarSurface);
    expect(light.muted, AppTheme.lightShellSurface);
    expect(light.accent, const Color(0xFFEDEDED));
    expect(AppTheme.navigationAccent, const Color(0xFF0285FF));
    expect(light.primary, const Color(0xFF1A1C1F));
    expect(light.mutedForeground, const Color(0xFF5D5D5D));
    expect(light.ring, Colors.transparent);
    expect(dark.background, const Color(0xFF181818));
    expect(dark.foreground, const Color(0xFFDFDFDF));
    expect(dark.card, const Color(0xFF181818));
    expect(dark.popover, AppTheme.darkShellSurface);
    expect(dark.secondary, AppTheme.darkSidebarSurface);
    expect(dark.muted, AppTheme.darkShellSurface);
    expect(dark.accent, const Color(0xFF282828));
    expect(dark.primary, const Color(0xFFDFDFDF));
    expect(dark.primaryForeground, const Color(0xFF181818));
    expect(dark.chart1, const Color(0xFFFA423E));
    expect(dark.chart2, const Color(0xFF04B84C));
    expect(dark.chart3, const Color(0xFFFB6A22));
    expect(dark.chart4, const Color(0xFF924FF7));
    expect(dark.ring, Colors.transparent);
  });

  test('keeps normal text color pairs at WCAG AA contrast', () {
    final light = AppTheme.light.colorScheme;
    final dark = AppTheme.dark.colorScheme;

    expect(_contrast(light.primary, light.primaryForeground), greaterThan(4.5));
    expect(_contrast(light.muted, light.mutedForeground), greaterThan(4.5));
    expect(_contrast(dark.primary, dark.primaryForeground), greaterThan(4.5));
    expect(_contrast(dark.muted, dark.mutedForeground), greaterThan(4.5));
  });

  test('uses native compact typography and standardized icon sizes', () {
    final typography = AppTheme.light.typography;
    final icons = AppTheme.light.iconTheme;

    expect(typography.sans.fontFamily, isNull);
    expect(typography.base.fontSize, 14);
    expect(typography.h1.fontSize, 24);
    expect(typography.h1.fontWeight, FontWeight.w600);
    expect(AppIconSize.xs, 12);
    expect(AppIconSize.sm, 16);
    expect(AppIconSize.md, 20);
    expect(AppIconSize.lg, 24);
    expect(AppIconSize.xl, 32);
    expect(AppIconSize.state, 48);
    expect(AppIconSize.hero, 64);
    expect(icons.xSmall.size, AppIconSize.xs);
    expect(icons.small.size, AppIconSize.sm);
    expect(icons.medium.size, AppIconSize.md);
    expect(icons.large.size, AppIconSize.lg);
    expect(icons.xLarge.size, AppIconSize.xl);
    expect(icons.x3Large.size, AppIconSize.state);
    expect(icons.x4Large.size, AppIconSize.hero);
  });

  testWidgets(
    'centers text line leading and button icons on every platform',
    (tester) async {
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(
            child: Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Text('Shared label'),
                  Button.primary(
                    onPressed: () {},
                    leading: const Icon(LucideIcons.copy),
                    child: const Text('Copy label'),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
      final labelStyle = DefaultTextStyle.of(
        tester.element(find.text('Shared label')),
      ).style;
      expect(labelStyle.leadingDistribution, TextLeadingDistribution.even);
      final buttonText = find.text('Copy label');
      final icon = find.byIcon(LucideIcons.copy);
      expect(
        tester.getRect(buttonText).center.dy,
        closeTo(tester.getRect(icon).center.dy, 0.01),
      );
      expect(
        DefaultTextStyle.of(
          tester.element(buttonText),
        ).style.leadingDistribution,
        TextLeadingDistribution.even,
      );
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  test('exposes the shared spacing, radius, and compact density contract', () {
    expect(AppSpacing.zero, 0);
    expect(AppSpacing.xxs, 2);
    expect(AppSpacing.xs, 4);
    expect(AppSpacing.sm, 8);
    expect(AppSpacing.md, 12);
    expect(AppSpacing.lg, 16);
    expect(AppSpacing.xl, 20);
    expect(AppSpacing.xxl, 24);
    expect(AppSpacing.xxxl, 32);
    expect(AppSpacing.huge, 40);
    expect(AppSpacing.massive, 48);
    expect(AppRadius.xs, 4);
    expect(AppRadius.sm, 6);
    expect(AppRadius.md, 8);
    expect(AppRadius.lg, 12);
    expect(AppRadius.xl, 16);
    expect(AppRadius.full, 999);
    expect(AppControlSize.compact, 32);
    expect(AppControlSize.regular, 36);
    expect(AppControlSize.large, 40);
    expect(AppControlSize.touch, 48);
    expect(AppOverlaySize.dialogMaxWidth, 480);
    expect(AppOverlaySize.drawerHeightFactor, 0.86);
    expect(AppOverlaySize.toastMaxWidth, 360);
    expect(AppOverlaySize.dragHandleWidth, 36);
    expect(AppOverlaySize.dragHandleHeight, 4);
    expect(AppTheme.light.radius, 8 / 9);
    expect(AppTheme.light.radiusMd, AppRadius.md);
    expect(AppTheme.light.density.baseContainerPadding, AppSpacing.lg);
    expect(AppTheme.light.density.baseGap, AppSpacing.xs);
    expect(AppTheme.light.density.baseContentPadding, AppSpacing.md);
  });

  testWidgets('installs borderless component themes for every state', (
    tester,
  ) async {
    FocusOutlineTheme? focusTheme;
    TextFieldTheme? textFieldTheme;
    SelectTheme? selectTheme;
    InputOTPTheme? inputOtpTheme;
    TooltipTheme? tooltipTheme;
    DrawerTheme? drawerTheme;
    ToastTheme? toastTheme;
    PrimaryButtonTheme? primaryButtonTheme;
    SecondaryButtonTheme? secondaryButtonTheme;
    OutlineButtonTheme? outlineButtonTheme;
    GhostButtonTheme? ghostButtonTheme;
    LinkButtonTheme? linkButtonTheme;
    TextButtonTheme? textButtonTheme;
    DestructiveButtonTheme? destructiveButtonTheme;
    FixedButtonTheme? fixedButtonTheme;
    MenuButtonTheme? menuButtonTheme;
    MenubarButtonTheme? menubarButtonTheme;
    MutedButtonTheme? mutedButtonTheme;
    CardButtonTheme? cardButtonTheme;
    CardTheme? cardTheme;
    late BuildContext themedContext;

    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
        builder: AppTheme.builder,
        home: Builder(
          builder: (context) {
            themedContext = context;
            focusTheme = ComponentTheme.maybeOf<FocusOutlineTheme>(context);
            textFieldTheme = ComponentTheme.maybeOf<TextFieldTheme>(context);
            selectTheme = ComponentTheme.maybeOf<SelectTheme>(context);
            inputOtpTheme = ComponentTheme.maybeOf<InputOTPTheme>(context);
            tooltipTheme = ComponentTheme.maybeOf<TooltipTheme>(context);
            drawerTheme = ComponentTheme.maybeOf<DrawerTheme>(context);
            toastTheme = ComponentTheme.maybeOf<ToastTheme>(context);
            primaryButtonTheme = ComponentTheme.maybeOf<PrimaryButtonTheme>(
              context,
            );
            secondaryButtonTheme = ComponentTheme.maybeOf<SecondaryButtonTheme>(
              context,
            );
            outlineButtonTheme = ComponentTheme.maybeOf<OutlineButtonTheme>(
              context,
            );
            ghostButtonTheme = ComponentTheme.maybeOf<GhostButtonTheme>(
              context,
            );
            linkButtonTheme = ComponentTheme.maybeOf<LinkButtonTheme>(context);
            textButtonTheme = ComponentTheme.maybeOf<TextButtonTheme>(context);
            destructiveButtonTheme =
                ComponentTheme.maybeOf<DestructiveButtonTheme>(context);
            fixedButtonTheme = ComponentTheme.maybeOf<FixedButtonTheme>(
              context,
            );
            menuButtonTheme = ComponentTheme.maybeOf<MenuButtonTheme>(context);
            menubarButtonTheme = ComponentTheme.maybeOf<MenubarButtonTheme>(
              context,
            );
            mutedButtonTheme = ComponentTheme.maybeOf<MutedButtonTheme>(
              context,
            );
            cardButtonTheme = ComponentTheme.maybeOf<CardButtonTheme>(context);
            cardTheme = ComponentTheme.maybeOf<CardTheme>(context);
            return const SizedBox.shrink();
          },
        ),
      ),
    );

    expect(focusTheme?.border?.top.style, BorderStyle.none);
    expect(textFieldTheme?.filled, isTrue);
    expect(textFieldTheme?.border?.top.style, BorderStyle.none);
    for (final states in <Set<WidgetState>>[
      const {},
      const {WidgetState.hovered},
      const {WidgetState.focused},
      const {WidgetState.pressed},
    ]) {
      final decoration =
          selectTheme!.decoration!(themedContext, states, const BoxDecoration())
              as BoxDecoration;
      expect(decoration.border?.top.style, BorderStyle.none);
    }
    expect(inputOtpTheme?.spacing, AppSpacing.sm);
    expect(inputOtpTheme?.height, AppControlSize.regular);
    expect(selectTheme?.adaptiveOverlay, isFalse);
    final selectOverlay =
        selectTheme?.overlayConfiguration as PopoverConfiguration?;
    expect(selectOverlay?.alignment, Alignment.topLeft);
    expect(selectOverlay?.anchorAlignment, Alignment.bottomLeft);
    expect(selectOverlay?.offset, const Offset(0, AppSpacing.xs));
    expect(selectOverlay?.widthConstraint, PopoverConstraint.anchorMinSize);
    expect(drawerTheme?.barrierColor, AppOverlays.scrimColor);
    expect(drawerTheme?.surfaceOpacity, 1);
    expect(drawerTheme?.surfaceBlur, 0);
    expect(drawerTheme?.showDragHandle, isTrue);
    expect(
      drawerTheme?.dragHandleSize,
      const Size(
        AppOverlaySize.dragHandleWidth,
        AppOverlaySize.dragHandleHeight,
      ),
    );
    expect(toastTheme?.padding, const EdgeInsets.all(AppSpacing.lg));
    expect(toastTheme?.spacing, AppSpacing.sm);
    expect(
      toastTheme?.toastConstraints?.maxWidth,
      AppOverlaySize.toastMaxWidth,
    );
    expect(tooltipTheme?.surfaceOpacity, 1);
    expect(tooltipTheme?.surfaceBlur, 0);
    expect(
      tooltipTheme?.padding,
      const EdgeInsets.symmetric(
        horizontal: AppSpacing.sm,
        vertical: AppSpacing.xs,
      ),
    );
    expect(tooltipTheme?.backgroundColor, AppTheme.light.colorScheme.primary);
    expect(
      tooltipTheme?.borderRadius,
      const BorderRadius.all(Radius.circular(AppRadius.sm)),
    );
    final buttonThemes = <ButtonTheme?>[
      primaryButtonTheme,
      secondaryButtonTheme,
      outlineButtonTheme,
      ghostButtonTheme,
      linkButtonTheme,
      textButtonTheme,
      destructiveButtonTheme,
      fixedButtonTheme,
      menuButtonTheme,
      menubarButtonTheme,
      mutedButtonTheme,
      cardButtonTheme,
    ];
    expect(buttonThemes, everyElement(isNotNull));
    for (final buttonTheme in buttonThemes.cast<ButtonTheme>()) {
      expect(
        buttonTheme.padding!(themedContext, const {}, EdgeInsets.zero),
        const EdgeInsets.symmetric(
          horizontal: AppSpacing.md,
          vertical: AppSpacing.sm,
        ),
      );
      final textStyle = buttonTheme.textStyle!(
        themedContext,
        const {},
        const TextStyle(color: Color(0xFF123456)),
      );
      expect(textStyle.color, const Color(0xFF123456));
      expect(
        textStyle.fontSize,
        Theme.of(themedContext).typography.textSmall.fontSize,
      );
      expect(textStyle.fontWeight, FontWeight.w500);
      expect(
        buttonTheme
            .iconTheme!(themedContext, const {}, const IconThemeData())
            .size,
        AppIconSize.sm,
      );
      expect(
        buttonTheme.margin!(themedContext, const {}, const EdgeInsets.all(99)),
        EdgeInsets.zero,
      );
    }
    expect(cardTheme?.duration, AppMotion.quick);
  });

  testWidgets('removes shared component motion when animations are disabled', (
    tester,
  ) async {
    tester.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(tester.platformDispatcher.clearAccessibilityFeaturesTestValue);
    CardTheme? cardTheme;

    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
        builder: AppTheme.builder,
        home: Builder(
          builder: (context) {
            cardTheme = ComponentTheme.maybeOf<CardTheme>(context);
            return const SizedBox.shrink();
          },
        ),
      ),
    );

    expect(cardTheme?.duration, Duration.zero);
  });

  testWidgets('renders the shared tooltip surface below its centered anchor', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(400, 300));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: ThemeMode.light,
        builder: AppTheme.builder,
        home: const Center(
          child: Tooltip(
            waitDuration: Duration.zero,
            tooltip: _testTooltip,
            child: SizedBox(
              key: ValueKey<String>('tooltip-anchor'),
              width: AppControlSize.regular,
              height: AppControlSize.regular,
              child: ColoredBox(color: Color(0x01000000)),
            ),
          ),
        ),
      ),
    );

    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(mouse.removePointer);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(
      tester.getCenter(find.byKey(const ValueKey<String>('tooltip-anchor'))),
    );
    await tester.pumpAndSettle();

    final anchorRect = tester.getRect(
      find.byKey(const ValueKey<String>('tooltip-anchor')),
    );
    final tooltipFinder = find.byType(TooltipContainer);
    final tooltipRect = tester.getRect(tooltipFinder);
    final surface = tester.widget<Container>(
      find.descendant(of: tooltipFinder, matching: find.byType(Container)),
    );
    final decoration = surface.decoration! as BoxDecoration;

    expect(find.text('Tooltip label'), findsOneWidget);
    expect(decoration.color, AppTheme.light.colorScheme.primary);
    expect(tooltipRect.top, greaterThanOrEqualTo(anchorRect.bottom));
    expect(tooltipRect.center.dx, closeTo(anchorRect.center.dx, 0.5));
  });

  test('production buttons use the shared shadcn Button API', () {
    final forbiddenButton = RegExp(
      r'\b(?:PrimaryButton|SecondaryButton|OutlineButton|GhostButton|LinkButton|TextButton|DestructiveButton|IconButton|SelectedButton|ElevatedButton|FilledButton|FloatingActionButton|CupertinoButton)(?:\.[A-Za-z0-9_]+)?\s*\(',
    );
    final offenders = <String>[];
    for (final entity in Directory('lib').listSync(recursive: true)) {
      if (entity is! File || !entity.path.endsWith('.dart')) continue;
      if (forbiddenButton.hasMatch(entity.readAsStringSync())) {
        offenders.add(entity.path);
      }
    }

    expect(
      offenders,
      isEmpty,
      reason:
          'All application buttons must use shadcn Button with a ButtonStyle modifier.',
    );
  });

  test('production UI keeps Lucide as its only application icon set', () {
    final forbiddenIconSet = RegExp(
      r'\b(?:Icons|CupertinoIcons|FontAwesomeIcons|RadixIcons|BootstrapIcons)\.',
    );
    final offenders = <String>[];
    for (final entity in Directory('lib').listSync(recursive: true)) {
      if (entity is! File || !entity.path.endsWith('.dart')) continue;
      if (forbiddenIconSet.hasMatch(entity.readAsStringSync())) {
        offenders.add(entity.path);
      }
    }

    expect(
      offenders,
      isEmpty,
      reason: 'Application UI icons must use LucideIcons from shadcn_flutter.',
    );
  });

  test('production overlays use the shared application contract', () {
    final localOverlayConstruction = RegExp(
      r'\b(?:AlertDialog|DialogConfiguration|DrawerConfiguration|PopoverConfiguration)\s*\(',
    );
    final offenders = <String>[];
    for (final entity in Directory('lib').listSync(recursive: true)) {
      if (entity is! File || !entity.path.endsWith('.dart')) continue;
      if (entity.path
          .replaceAll('\\', '/')
          .endsWith('/app/theme/app_overlays.dart')) {
        continue;
      }
      if (localOverlayConstruction.hasMatch(entity.readAsStringSync())) {
        offenders.add(entity.path);
      }
    }

    expect(
      offenders,
      isEmpty,
      reason:
          'Application-owned overlays must use AppOverlays instead of local visual configuration.',
    );
  });

  test('production toasts use the single application toast component', () {
    final directToastCall = RegExp(r'\bshowToast\s*\(');
    final offenders = <String>[];
    for (final entity in Directory('lib').listSync(recursive: true)) {
      if (entity is! File || !entity.path.endsWith('.dart')) continue;
      if (entity.path
          .replaceAll('\\', '/')
          .endsWith('/app/theme/app_toast.dart')) {
        continue;
      }
      if (directToastCall.hasMatch(entity.readAsStringSync())) {
        offenders.add(entity.path);
      }
    }

    expect(
      offenders,
      isEmpty,
      reason: 'Application notifications must use AppToast.',
    );
  });
}

Widget _testTooltip(BuildContext context) {
  return const TooltipContainer(child: Text('Tooltip label'));
}

double _contrast(Color first, Color second) {
  final lightest = [first.computeLuminance(), second.computeLuminance()]
    ..sort();
  return (lightest.last + 0.05) / (lightest.first + 0.05);
}
