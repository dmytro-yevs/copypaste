import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_tokens.dart';
import 'devices_gateway.dart';

/// The shared visual mapping for typed device metadata.
abstract final class DevicePresentation {
  static const IconData collectionIcon = LucideIcons.monitorSmartphone;

  static String connectionLabel(DeviceConnectionState state) => switch (state) {
    DeviceConnectionState.online => 'Online',
    DeviceConnectionState.offline => 'Offline',
    DeviceConnectionState.visible => 'Visible',
    DeviceConnectionState.notSeen => 'Not seen',
    DeviceConnectionState.unknown => 'Unknown',
  };

  static AppStatusTone connectionTone(DeviceConnectionState state) =>
      switch (state) {
        DeviceConnectionState.online => AppStatusTone.success,
        DeviceConnectionState.offline => AppStatusTone.error,
        DeviceConnectionState.visible => AppStatusTone.info,
        DeviceConnectionState.notSeen ||
        DeviceConnectionState.unknown => AppStatusTone.muted,
      };

  static IconData pairingEntryIcon(PairingEntryMode? mode) => switch (mode) {
    PairingEntryMode.invite => LucideIcons.qrCode,
    PairingEntryMode.scanQr => LucideIcons.scanLine,
    PairingEntryMode.enterCode => LucideIcons.keyboard,
    PairingEntryMode.nearby || null => LucideIcons.link,
  };

  static IconData pairingStateIcon(PairingState state) => switch (state) {
    PairingState.waitingForPeer => LucideIcons.qrCode,
    PairingState.handshaking => LucideIcons.loaderCircle,
    PairingState.awaitingConfirmation => LucideIcons.shieldQuestion,
    PairingState.confirmed => LucideIcons.badgeCheck,
    PairingState.rejected || PairingState.failed => LucideIcons.circleX,
    PairingState.cancelled || PairingState.timedOut => LucideIcons.clockAlert,
    PairingState.idle => LucideIcons.link,
  };

  static String pairingStateTitle(PairingState state) => switch (state) {
    PairingState.waitingForPeer => 'Pair a device',
    PairingState.handshaking => 'Verifying device',
    PairingState.awaitingConfirmation => 'Confirm on both devices',
    PairingState.confirmed => 'Device paired',
    PairingState.rejected => 'Pairing rejected',
    PairingState.cancelled => 'Pairing cancelled',
    PairingState.timedOut => 'Pairing timed out',
    PairingState.failed => 'Pairing failed',
    PairingState.idle => 'Pairing',
  };

  static String pairingStateDescription(PairingState state) => switch (state) {
    PairingState.waitingForPeer =>
      'Reveal a QR code, scan one, or enter a pairing code in this protected window.',
    PairingState.handshaking =>
      'The devices are establishing a secure channel.',
    PairingState.awaitingConfirmation =>
      'Compare the protected verification code, then confirm on both devices.',
    PairingState.confirmed =>
      'The device is connected. Your group membership will sync automatically.',
    PairingState.rejected ||
    PairingState.cancelled ||
    PairingState.failed => 'No device was added.',
    PairingState.timedOut => 'Start a new pairing attempt to continue.',
    PairingState.idle => 'Start a pairing attempt to connect a device.',
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
    DevicePlatform.linux => 'Linux',
  };
}
