import 'dart:io';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

const _resources = 'android/app/src/main/res';

Map<String, String> _values(String path, String tag) {
  final source = File('$_resources/$path').readAsStringSync();
  return {
    for (final match in RegExp(
      '<$tag name="([a-z0-9_]+)">([^<]+)</$tag>',
    ).allMatches(source))
      match.group(1)!: match.group(2)!.trim(),
  };
}

String _dp(double value) => '${_number(value)}dp';

String _sp(double? value) => '${_number(value!)}sp';

String _number(double value) =>
    value == value.roundToDouble() ? '${value.toInt()}' : '$value';

String _argb(Color color) =>
    '#${color.toARGB32().toRadixString(16).padLeft(8, '0').toUpperCase()}';

Map<String, String> _colors(ThemeData theme, {required bool dark}) {
  final colors = theme.colorScheme;
  return {
    'app_color_background': _argb(colors.background),
    'app_color_foreground': _argb(colors.foreground),
    'app_color_card': _argb(colors.card),
    'app_color_card_foreground': _argb(colors.cardForeground),
    'app_color_popover': _argb(colors.popover),
    'app_color_popover_foreground': _argb(colors.popoverForeground),
    'app_color_primary': _argb(colors.primary),
    'app_color_primary_foreground': _argb(colors.primaryForeground),
    'app_color_secondary': _argb(colors.secondary),
    'app_color_secondary_foreground': _argb(colors.secondaryForeground),
    'app_color_muted': _argb(colors.muted),
    'app_color_muted_foreground': _argb(colors.mutedForeground),
    'app_color_accent': _argb(colors.accent),
    'app_color_accent_foreground': _argb(colors.accentForeground),
    'app_color_destructive': _argb(colors.destructive),
    'app_color_border': _argb(colors.border),
    'app_color_input': _argb(colors.input),
    'app_color_status_warning': _argb(
      dark ? AppStatusColor.warningDark : AppStatusColor.warningLight,
    ),
    'app_color_status_info': _argb(
      dark ? AppStatusColor.infoDark : AppStatusColor.infoLight,
    ),
    'app_color_status_success': _argb(
      dark ? AppStatusColor.successDark : AppStatusColor.successLight,
    ),
  };
}

void main() {
  test('Android dimension tokens mirror the shared Flutter tokens', () {
    final typography = AppTheme.light.typography;
    expect(_values('values/design_tokens.xml', 'dimen'), {
      'app_spacing_xxs': _dp(AppSpacing.xxs),
      'app_spacing_xs': _dp(AppSpacing.xs),
      'app_spacing_sm': _dp(AppSpacing.sm),
      'app_spacing_md': _dp(AppSpacing.md),
      'app_spacing_lg': _dp(AppSpacing.lg),
      'app_spacing_xl': _dp(AppSpacing.xl),
      'app_spacing_xxl': _dp(AppSpacing.xxl),
      'app_spacing_xxxl': _dp(AppSpacing.xxxl),
      'app_spacing_huge': _dp(AppSpacing.huge),
      'app_spacing_massive': _dp(AppSpacing.massive),
      'app_icon_size_status_indicator': _dp(AppIconSize.statusIndicator),
      'app_icon_size_xs': _dp(AppIconSize.xs),
      'app_icon_size_sm': _dp(AppIconSize.sm),
      'app_icon_size_md': _dp(AppIconSize.md),
      'app_icon_size_lg': _dp(AppIconSize.lg),
      'app_icon_size_xl': _dp(AppIconSize.xl),
      'app_icon_size_state': _dp(AppIconSize.state),
      'app_icon_size_hero': _dp(AppIconSize.hero),
      'app_radius_xs': _dp(AppRadius.xs),
      'app_radius_sm': _dp(AppRadius.sm),
      'app_radius_md': _dp(AppRadius.md),
      'app_radius_lg': _dp(AppRadius.lg),
      'app_radius_xl': _dp(AppRadius.xl),
      'app_radius_full': _dp(AppRadius.full),
      'app_control_size_compact': _dp(AppControlSize.compact),
      'app_control_size_regular': _dp(AppControlSize.regular),
      'app_control_size_large': _dp(AppControlSize.large),
      'app_control_size_touch': _dp(AppControlSize.touch),
      'app_control_size_navigation': _dp(AppControlSize.navigation),
      'app_text_size_x_small': _sp(typography.xSmall.fontSize),
      'app_text_size_small': _sp(typography.small.fontSize),
      'app_text_size_base': _sp(typography.base.fontSize),
      'app_text_size_large': _sp(typography.large.fontSize),
      'app_text_size_x_large': _sp(typography.xLarge.fontSize),
      'app_text_size_history_metadata': _sp(AppTypographySize.historyMetadata),
      'app_text_size_menu': _sp(AppTypographySize.menu),
      'app_text_size_menu_metadata': _sp(AppTypographySize.menuMetadata),
    });
  });

  test('Android light and night colors mirror the app color schemes', () {
    expect(
      _values('values/design_tokens.xml', 'color'),
      _colors(AppTheme.light, dark: false),
    );
    expect(
      _values('values-night/design_tokens.xml', 'color'),
      _colors(AppTheme.dark, dark: true),
    );
    expect(_values('values-night/design_tokens.xml', 'dimen'), isEmpty);
  });
}
