import 'package:copypaste_flutter/app/navigation/mobile_navigation_layout.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('caps the capsule and gives spare width to short labels', (
    _,
  ) async {
    final layout = MobileNavigationLayout.resolve(
      availableWidth: 600,
      labels: const ['A', 'B', 'Long label'],
      textStyle: AppTheme.mobileNavigationLabelStyle,
      textScaler: TextScaler.noScaling,
      textDirection: TextDirection.ltr,
    );
    expect(layout.width, closeTo(328, 0.001));
    expect(layout.height, 56);
    expect(layout.fontSize, 12);
    expect(layout.widths[0], greaterThanOrEqualTo(48));
    expect(layout.widths[2], greaterThan(layout.widths[0]));
  });

  testWidgets('fits long labels by using the compact typography pass', (
    _,
  ) async {
    final layout = MobileNavigationLayout.resolve(
      availableWidth: 280,
      labels: const [
        'Long history name',
        'Long device name',
        'Long settings name',
      ],
      textStyle: AppTheme.mobileNavigationLabelStyle,
      textScaler: TextScaler.noScaling,
      textDirection: TextDirection.rtl,
    );
    expect(layout.fontSize, 10);
    expect(layout.width, closeTo(280, 0.001));
    expect(layout.height, 56);
    expect(layout.positionAt(layout.centerAt(0)), closeTo(0, 0.001));
    expect(layout.positionAt(-100), 2);
    expect(layout.positionAt(400), 0);
  });

  testWidgets('keeps accessibility text scaling and grows the capsule height', (
    _,
  ) async {
    final layout = MobileNavigationLayout.resolve(
      availableWidth: 304,
      labels: const ['History', 'Devices', 'Settings'],
      textStyle: AppTheme.mobileNavigationLabelStyle,
      textScaler: const TextScaler.linear(2),
      textDirection: TextDirection.ltr,
    );
    expect(layout.fontSize, 12);
    expect(layout.width, closeTo(304, 0.001));
    expect(layout.height, greaterThan(56));
    expect(layout.positionAt(-100), 0);
    expect(layout.positionAt(400), 2);
    for (var index = 0; index < 3; index++) {
      expect(layout.positionAt(layout.centerAt(index)), closeTo(index, 0.001));
    }
  });
}
