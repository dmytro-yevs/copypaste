import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_tokens.dart';
import 'devices_gateway.dart';

/// The shared visual mapping for typed device metadata.
abstract final class DevicePresentation {
  static const IconData collectionIcon = LucideIcons.monitorSmartphone;

  static String connectionLabel(DeviceConnectionState state) {
    final strings = AppStrings.current;
    return switch (state) {
      DeviceConnectionState.online => strings.devicesConnectionOnline,
      DeviceConnectionState.offline => strings.devicesConnectionOffline,
      DeviceConnectionState.visible => strings.devicesConnectionVisible,
      DeviceConnectionState.notSeen => strings.devicesConnectionNotSeen,
      DeviceConnectionState.unknown => strings.devicesConnectionUnknown,
    };
  }

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

  static String pairingStateTitle(PairingState state) {
    final strings = AppStrings.current;
    return switch (state) {
      PairingState.waitingForPeer => strings.pairingPairDevice,
      PairingState.handshaking => strings.pairingStateHandshaking,
      PairingState.awaitingConfirmation =>
        strings.pairingStateAwaitingConfirmation,
      PairingState.confirmed => strings.pairingStateConfirmed,
      PairingState.rejected => strings.pairingStateRejected,
      PairingState.cancelled => strings.pairingStateCancelled,
      PairingState.timedOut => strings.pairingStateTimedOut,
      PairingState.failed => strings.pairingStateFailed,
      PairingState.idle => strings.pairingTitle,
    };
  }

  static String pairingStateDescription(PairingState state) {
    final strings = AppStrings.current;
    return switch (state) {
      PairingState.waitingForPeer => strings.pairingProtectedWaitingDescription,
      PairingState.handshaking => strings.pairingHandshakingDescription,
      PairingState.awaitingConfirmation =>
        strings.pairingAwaitingConfirmationDescription,
      PairingState.confirmed => strings.pairingConfirmedDescription,
      PairingState.rejected ||
      PairingState.cancelled ||
      PairingState.failed => strings.pairingNoDeviceAdded,
      PairingState.timedOut => strings.pairingTimedOutDescription,
      PairingState.idle => strings.pairingIdleDescription,
    };
  }

  static IconData icon(DeviceClass? deviceClass) => switch (deviceClass) {
    DeviceClass.desktop => LucideIcons.monitor,
    DeviceClass.laptop => LucideIcons.laptop,
    DeviceClass.phone => LucideIcons.smartphone,
    DeviceClass.tablet => LucideIcons.tablet,
    DeviceClass.unknown || null => LucideIcons.circleHelp,
  };

  static String summary(DeviceDetails? details) =>
      '${classLabel(details?.profile?.deviceClass)} · ${osLabel(details?.profile)}';

  static String classLabel(DeviceClass? deviceClass) {
    final strings = AppStrings.current;
    return switch (deviceClass) {
      DeviceClass.desktop => strings.devicesClassDesktop,
      DeviceClass.laptop => strings.devicesClassLaptop,
      DeviceClass.phone => strings.devicesClassPhone,
      DeviceClass.tablet => strings.devicesClassTablet,
      DeviceClass.unknown || null => strings.devicesClassUnknown,
    };
  }

  static String osLabel(DeviceProfile? profile) {
    if (profile == null) return AppStrings.current.devicesUnknownOs;
    final name = profile.osName ?? platformLabel(profile.platform);
    final version = profile.osVersion;
    return version == null || version.isEmpty ? name : '$name $version';
  }

  static String platformLabel(DevicePlatform platform) => switch (platform) {
    DevicePlatform.macos => 'macOS',
    DevicePlatform.windows => 'Windows',
    DevicePlatform.android => 'Android',
    DevicePlatform.unknown => AppStrings.current.devicesUnknownOs,
    DevicePlatform.linux => 'Linux',
  };
}
