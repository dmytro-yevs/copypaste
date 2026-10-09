import 'package:shadcn_flutter/shadcn_flutter.dart';

enum AppStatusTone { muted, info, success, error }

abstract final class AppStatusColor {
  static const infoLight = Color(0xFF2563EB);
  static const infoDark = Color(0xFF60A5FA);
  static const successLight = Color(0xFF15803D);
  static const successDark = Color(0xFF4ADE80);
}

/// Shared layout and icon sizing tokens for CopyPaste UI.
///
/// Values follow a compact 4px rhythm so every supported platform uses the
/// same visual cadence.
abstract final class AppSpacing {
  static const double zero = 0;
  static const double xxs = 2;
  static const double xs = 4;
  static const double sm = 8;
  static const double md = 12;
  static const double lg = 16;
  static const double xl = 20;
  static const double xxl = 24;
  static const double xxxl = 32;
  static const double huge = 40;
  static const double massive = 48;
  static const double navigationIconLabelGap = 1 / 3;
  static const double navigationShadowBlur = 8 / 3;
  static const double navigationShadowOffset = 0.85;
}

/// Standard Lucide icon sizes for CopyPaste UI.
abstract final class AppIconSize {
  static const double xs = 12;
  static const double sm = 16;
  static const double md = 20;
  static const double lg = 24;
  static const double xl = 32;
  static const double state = 48;
  static const double hero = 64;
}

/// Shared corner radii for CopyPaste UI.
abstract final class AppRadius {
  static const double xs = 4;
  static const double sm = 6;
  static const double md = 8;
  static const double lg = 12;
  static const double xl = 16;
  static const double navigation = 28;
  static const double full = 999;
}

/// Standard interactive control heights for CopyPaste UI.
abstract final class AppControlSize {
  static const double compact = 32;
  static const double regular = 36;
  static const double large = 40;
  static const double touch = 48;
  static const double navigation = 56;
  static const double navigationLabelHeight =
      touch -
      AppSpacing.xs * 2 -
      AppIconSize.lg -
      AppSpacing.navigationIconLabelGap;
}

/// Shared compact metadata and desktop menu typography.
abstract final class AppTypographySize {
  static const double historyMetadata = 12;
  static const double menu = 13;
  static const double menuMetadata = menu - 2;
  static const double navigation = 12;
  static const double navigationCompact = 10;
}

/// Shared dimensions for application layouts.
abstract final class AppLayoutSize {
  static const double inspectorLabelMaxWidth = 160;
  static const double inspectorLabelWidthFactor = 1 / 3;
  static const double historySearchMinWidth = 160;
  static const double onboardingContentMaxWidth = 440;
  static const double marketplaceCardMinWidth = 280;
  static const double quickPasteMenuWidth = 448;
  static const double quickPasteInspectorWidth = 360;
  static const double settingsStackedControlWidth = 420;
  static const double settingsNavigationWidth = 240;
  static const double settingsContentMaxWidth = 900;
  static const double mobileNavigationMaxWidth = 328;
}

/// Shared dimensions for application-owned overlays.
abstract final class AppOverlaySize {
  static const double dialogMaxWidth = 480;
  static const double dialogContentHeightFactor = 0.5;
  static const double drawerHeightFactor = 0.86;
  static const double drawerPanelWidth = 480;
  static const double toastMaxWidth = 360;
  static const double dragHandleWidth = 36;
  static const double dragHandleHeight = 4;
}
