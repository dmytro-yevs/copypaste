import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'devices_gateway.dart';

/// The shared visual mapping for typed device metadata.
abstract final class DevicePresentation {
  static IconData pairingEntryIcon(PairingEntryMode? mode) => switch (mode) {
    PairingEntryMode.invite => LucideIcons.qrCode,
    PairingEntryMode.scanQr => LucideIcons.scanLine,
    PairingEntryMode.enterCode => LucideIcons.keyboard,
    null => LucideIcons.link,
  };

  static IconData icon(DeviceClass? deviceClass) => switch (deviceClass) {
    DeviceClass.desktop => LucideIcons.monitor,
    DeviceClass.laptop => LucideIcons.laptop,
    DeviceClass.phone => LucideIcons.smartphone,
    DeviceClass.tablet => LucideIcons.tablet,
    DeviceClass.unknown || null => LucideIcons.circleHelp,
  };

  static String summary(DeviceDetails? details) =>
      '${classLabel(details?.profile?.deviceClass)} · ${osLabel(details?.profile)}';

  static String classLabel(DeviceClass? deviceClass) => switch (deviceClass) {
    DeviceClass.desktop => 'Desktop',
    DeviceClass.laptop => 'Laptop',
    DeviceClass.phone => 'Phone',
    DeviceClass.tablet => 'Tablet',
    DeviceClass.unknown || null => 'Device',
  };

  static String osLabel(DeviceProfile? profile) {
    if (profile == null) return 'Unknown OS';
    final name = profile.osName ?? platformLabel(profile.platform);
    final version = profile.osVersion;
    return version == null || version.isEmpty ? name : '$name $version';
  }

  static String platformLabel(DevicePlatform platform) => switch (platform) {
    DevicePlatform.macos => 'macOS',
    DevicePlatform.windows => 'Windows',
    DevicePlatform.android => 'Android',
    DevicePlatform.unknown => 'Unknown OS',
  };
}
