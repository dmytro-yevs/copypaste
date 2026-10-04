import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'app_motion.dart';
import 'app_overlays.dart';
import 'app_tokens.dart';

abstract final class AppTheme {
  static const mode = ThemeMode.system;

  static const navigationAccent = Color(0xFF0285FF);
  static const lightShellSurface = Color(0xFFEDEDED);
  static const darkShellSurface = Color(0xFF212121);
  static const lightSidebarSurface = Color(0xFFF9F9F9);
  static const darkSidebarSurface = Color(0xFF1B1B1B);

  static CardTheme settingsSearchTargetCardTheme(
    BuildContext context, {
    required bool highlighted,
  }) {
    return CardTheme(
      filled: highlighted,
      fillColor: Theme.of(context).colorScheme.accent,
      duration: AppMotion.resolve(context, AppMotion.quick),
    );
  }

  /// The shared ChatGPT application light palette.
  static const _lightColors = ColorScheme(
    brightness: Brightness.light,
    background: Color(0xFFFFFFFF),
    foreground: Color(0xFF1A1C1F),
    card: Color(0xFFFFFFFF),
    cardForeground: Color(0xFF1A1C1F),
    popover: Color(0xFFFFFFFF),
    popoverForeground: Color(0xFF1A1C1F),
    primary: Color(0xFF1A1C1F),
    primaryForeground: Color(0xFFFFFFFF),
    secondary: lightSidebarSurface,
    secondaryForeground: Color(0xFF1A1C1F),
    muted: lightShellSurface,
    mutedForeground: Color(0xFF5D5D5D),
    accent: Color(0xFFEDEDED),
    accentForeground: Color(0xFF1A1C1F),
    destructive: Color(0xFFE02E2A),
    border: Color(0x141A1C1F),
    input: Color(0xFFEDEDED),
    ring: Colors.transparent,
    chart1: Color(0xFFFA423E),
    chart2: Color(0xFF04B84C),
    chart3: Color(0xFFFB6A22),
    chart4: Color(0xFF924FF7),
    chart5: navigationAccent,
  );

  /// The shared ChatGPT application dark palette.
  static const _darkColors = ColorScheme(
    brightness: Brightness.dark,
    background: Color(0xFF181818),
    foreground: Color(0xFFDFDFDF),
    card: Color(0xFF181818),
    cardForeground: Color(0xFFDFDFDF),
    popover: darkShellSurface,
    popoverForeground: Color(0xFFDFDFDF),
    primary: Color(0xFFDFDFDF),
    primaryForeground: Color(0xFF181818),
    secondary: darkSidebarSurface,
    secondaryForeground: Color(0xFFFFFFFF),
    muted: darkShellSurface,
    mutedForeground: Color(0xFFAFAFAF),
    accent: Color(0xFF282828),
    accentForeground: Color(0xFFFFFFFF),
    destructive: Color(0xFFE02E2A),
    border: Color(0x14DFDFDF),
    input: darkShellSurface,
    ring: Colors.transparent,
    chart1: Color(0xFFFA423E),
    chart2: Color(0xFF04B84C),
    chart3: Color(0xFFFB6A22),
    chart4: Color(0xFF924FF7),
    chart5: navigationAccent,
  );

  static final _typography = const Typography.geist().copyWith(
    sans: () => const TextStyle(),
    mono: () => const TextStyle(fontFamily: 'monospace'),
    xSmall: () => const TextStyle(fontSize: 12, height: 4 / 3),
    small: () => const TextStyle(fontSize: 14, height: 10 / 7),
    base: () => const TextStyle(fontSize: 14, height: 10 / 7),
    large: () => const TextStyle(fontSize: 16, height: 1.5),
    xLarge: () => const TextStyle(fontSize: 18, height: 4 / 3),
    x2Large: () => const TextStyle(fontSize: 20, height: 1.3),
    x3Large: () => const TextStyle(fontSize: 24, height: 4 / 3),
    x4Large: () => const TextStyle(fontSize: 30, height: 1.2),
    h1: () => const TextStyle(
      fontSize: 24,
      height: 4 / 3,
      fontWeight: FontWeight.w600,
    ),
    h2: () =>
        const TextStyle(fontSize: 20, height: 1.3, fontWeight: FontWeight.w600),
    h3: () => const TextStyle(
      fontSize: 18,
      height: 4 / 3,
      fontWeight: FontWeight.w600,
    ),
    h4: () =>
        const TextStyle(fontSize: 16, height: 1.5, fontWeight: FontWeight.w600),
    p: () => const TextStyle(fontSize: 14, height: 10 / 7),
    lead: () => const TextStyle(fontSize: 18, height: 4 / 3),
    textLarge: () =>
        const TextStyle(fontSize: 16, height: 1.5, fontWeight: FontWeight.w600),
    textSmall: () => const TextStyle(
      fontSize: 14,
      height: 10 / 7,
      fontWeight: FontWeight.w500,
    ),
    textMuted: () => const TextStyle(fontSize: 14, height: 10 / 7),
    inlineCode: () => const TextStyle(
      fontFamily: 'monospace',
      fontSize: 12,
      height: 4 / 3,
      fontWeight: FontWeight.w500,
    ),
  );

  static const _iconTheme = IconThemeProperties(
    xSmall: IconThemeData(size: AppIconSize.xs),
    small: IconThemeData(size: AppIconSize.sm),
    medium: IconThemeData(size: AppIconSize.md),
    large: IconThemeData(size: AppIconSize.lg),
    xLarge: IconThemeData(size: AppIconSize.xl),
    x3Large: IconThemeData(size: AppIconSize.state),
    x4Large: IconThemeData(size: AppIconSize.hero),
  );

  static const _density = Density(
    baseContainerPadding: AppSpacing.lg,
    baseGap: AppSpacing.xs,
    baseContentPadding: AppSpacing.md,
  );

  static const _primaryButtonTheme = PrimaryButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _secondaryButtonTheme = SecondaryButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _outlineButtonTheme = OutlineButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _ghostButtonTheme = GhostButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _linkButtonTheme = LinkButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _textButtonTheme = TextButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _destructiveButtonTheme = DestructiveButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _fixedButtonTheme = FixedButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _menuButtonTheme = MenuButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _menubarButtonTheme = MenubarButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _mutedButtonTheme = MutedButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static const _cardButtonTheme = CardButtonTheme(
    padding: _buttonPadding,
    textStyle: _buttonTextStyle,
    iconTheme: _buttonIconTheme,
    margin: _buttonMargin,
  );

  static final light = ThemeData(
    colorScheme: _lightColors,
    typography: _typography,
    iconTheme: _iconTheme,
    radius: 8 / 9,
    density: _density,
  );

  static final dark = ThemeData.dark(
    colorScheme: _darkColors,
    typography: _typography,
    iconTheme: _iconTheme,
    radius: 8 / 9,
    density: _density,
  );

  /// Installs shared stock shadcn component styling for every app surface.
  static Widget builder(BuildContext context, Widget? child) {
    final cardTheme = CardTheme(
      duration: AppMotion.resolve(context, AppMotion.quick),
    );
    final tooltipTheme = TooltipTheme(
      surfaceOpacity: 1,
      surfaceBlur: 0,
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.sm,
        vertical: AppSpacing.xs,
      ),
      backgroundColor: Theme.of(context).colorScheme.primary,
      borderRadius: const BorderRadius.all(Radius.circular(AppRadius.sm)),
    );
    const drawerTheme = DrawerTheme(
      surfaceOpacity: 1,
      surfaceBlur: 0,
      barrierColor: AppOverlays.scrimColor,
      showDragHandle: true,
      dragHandleSize: Size(
        AppOverlaySize.dragHandleWidth,
        AppOverlaySize.dragHandleHeight,
      ),
    );
    final toastTheme = ToastTheme(
      padding: const EdgeInsets.all(AppSpacing.lg),
      expandingCurve: AppMotion.enterCurve,
      expandingDuration: AppMotion.resolve(context, AppMotion.standard),
      spacing: AppSpacing.sm,
      toastConstraints: const BoxConstraints(
        maxWidth: AppOverlaySize.toastMaxWidth,
      ),
    );
    final buttonThemedChild = ComponentTheme<PrimaryButtonTheme>(
      data: _primaryButtonTheme,
      child: ComponentTheme<SecondaryButtonTheme>(
        data: _secondaryButtonTheme,
        child: ComponentTheme<OutlineButtonTheme>(
          data: _outlineButtonTheme,
          child: ComponentTheme<GhostButtonTheme>(
            data: _ghostButtonTheme,
            child: ComponentTheme<LinkButtonTheme>(
              data: _linkButtonTheme,
              child: ComponentTheme<TextButtonTheme>(
                data: _textButtonTheme,
                child: ComponentTheme<DestructiveButtonTheme>(
                  data: _destructiveButtonTheme,
                  child: ComponentTheme<FixedButtonTheme>(
                    data: _fixedButtonTheme,
                    child: ComponentTheme<MenuButtonTheme>(
                      data: _menuButtonTheme,
                      child: ComponentTheme<MenubarButtonTheme>(
                        data: _menubarButtonTheme,
                        child: ComponentTheme<MutedButtonTheme>(
                          data: _mutedButtonTheme,
                          child: ComponentTheme<CardButtonTheme>(
                            data: _cardButtonTheme,
                            child: ComponentTheme<CardTheme>(
                              data: cardTheme,
                              child: ComponentTheme<DrawerTheme>(
                                data: drawerTheme,
                                child: ComponentTheme<ToastTheme>(
                                  data: toastTheme,
                                  child: child ?? const SizedBox.shrink(),
                                ),
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    return ComponentTheme<TooltipTheme>(
      data: tooltipTheme,
      child: ComponentTheme<FocusOutlineTheme>(
        data: const FocusOutlineTheme(
          border: Border.fromBorderSide(BorderSide.none),
        ),
        child: ComponentTheme<TextFieldTheme>(
          data: const TextFieldTheme(
            filled: true,
            border: Border.fromBorderSide(BorderSide.none),
            borderRadius: BorderRadius.all(Radius.circular(AppRadius.md)),
            padding: EdgeInsets.symmetric(
              horizontal: AppSpacing.md,
              vertical: AppSpacing.sm,
            ),
          ),
          child: ComponentTheme<SelectTheme>(
            data: SelectTheme(
              adaptiveOverlay: false,
              overlayConfiguration: AppOverlays.selectPopoverConfiguration(
                context,
              ),
              borderRadius: const BorderRadius.all(
                Radius.circular(AppRadius.md),
              ),
              padding: const EdgeInsets.symmetric(
                horizontal: AppSpacing.md,
                vertical: AppSpacing.sm,
              ),
              decoration: softSelectDecoration,
            ),
            child: ComponentTheme<InputOTPTheme>(
              data: const InputOTPTheme(
                spacing: AppSpacing.sm,
                height: AppControlSize.regular,
              ),
              child: buttonThemedChild,
            ),
          ),
        ),
      ),
    );
  }

  static EdgeInsetsGeometry _buttonPadding(
    BuildContext context,
    Set<WidgetState> states,
    EdgeInsetsGeometry value,
  ) {
    return const EdgeInsets.symmetric(
      horizontal: AppSpacing.md,
      vertical: AppSpacing.sm,
    );
  }

  static TextStyle _buttonTextStyle(
    BuildContext context,
    Set<WidgetState> states,
    TextStyle value,
  ) {
    final shared = Theme.of(context).typography.textSmall;
    return value.copyWith(
      fontFamily: shared.fontFamily,
      fontFamilyFallback: shared.fontFamilyFallback,
      fontSize: shared.fontSize,
      fontWeight: shared.fontWeight,
      height: shared.height,
    );
  }

  static IconThemeData _buttonIconTheme(
    BuildContext context,
    Set<WidgetState> states,
    IconThemeData value,
  ) {
    return value.copyWith(size: AppIconSize.sm);
  }

  static EdgeInsetsGeometry _buttonMargin(
    BuildContext context,
    Set<WidgetState> states,
    EdgeInsetsGeometry value,
  ) {
    return EdgeInsets.zero;
  }

  static Decoration softSelectDecoration(
    BuildContext context,
    Set<WidgetState> states,
    Decoration value,
  ) {
    final colors = Theme.of(context).colorScheme;
    final surface = states.contains(WidgetState.pressed)
        ? colors.accent
        : states.contains(WidgetState.hovered)
        ? colors.muted
        : colors.secondary;
    return BoxDecoration(
      color: surface,
      border: Border.fromBorderSide(BorderSide.none),
      borderRadius: const BorderRadius.all(Radius.circular(AppRadius.md)),
    );
  }
}
