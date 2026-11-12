import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_tokens.dart';
import '../../shared/step_progress.dart';
import 'devices_gateway.dart';
import 'pairing_progress.dart';

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

  /// The title of an ended ceremony, which never names who ended it.
  static String pairingOutcomeTitle(PairingProgress progress) =>
      progress.endedUnattributed
      ? AppStrings.current.pairingStateNotCompleted
      : pairingStateTitle(progress.outcome!);

  static StepProgress pairingStepProgress(
    PairingProgress progress, {
    Key? key,
  }) => StepProgress(
    key: key,
    steps: [for (final step in progress.steps) pairingStepLabel(step)],
    current: progress.activeIndex,
    // Each state shows its own activity in the pane; the steps only orient.
    running: false,
    failure: progress.failed ? pairingOutcomeTitle(progress) : null,
  );

  static String pairingStepLabel(PairingStep step) {
    final strings = AppStrings.current;
    return switch (step) {
      PairingStep.invite => strings.pairingStepInvite,
      PairingStep.scan => strings.pairingStepScan,
      PairingStep.code => strings.pairingStepCode,
      PairingStep.request => strings.pairingStepRequest,
      PairingStep.connect => strings.pairingStepConnect,
      PairingStep.confirm => strings.pairingStepConfirm,
      PairingStep.paired => strings.pairingStepPaired,
    };
  }

  static IconData pairingLocalDecisionIcon(PairingLocalDecision decision) =>
      switch (decision) {
        PairingLocalDecision.undecided => LucideIcons.circle,
        PairingLocalDecision.sending => LucideIcons.circleEllipsis,
        PairingLocalDecision.confirmed => LucideIcons.circleCheck,
        PairingLocalDecision.rejected => LucideIcons.circleX,
      };

  static String pairingLocalDecisionLabel(PairingLocalDecision decision) {
    final strings = AppStrings.current;
    return switch (decision) {
      PairingLocalDecision.undecided => strings.pairingDecisionNotConfirmed,
      PairingLocalDecision.sending => strings.pairingDecisionSending,
      PairingLocalDecision.confirmed => strings.pairingDecisionConfirmed,
      PairingLocalDecision.rejected => strings.pairingDecisionRejected,
    };
  }

  static AppStatusTone pairingLocalDecisionTone(
    PairingLocalDecision decision,
  ) => switch (decision) {
    PairingLocalDecision.confirmed => AppStatusTone.success,
    PairingLocalDecision.rejected => AppStatusTone.error,
    PairingLocalDecision.undecided ||
    PairingLocalDecision.sending => AppStatusTone.muted,
  };

  static IconData pairingPeerDecisionIcon(PairingPeerDecision decision) =>
      switch (decision) {
        PairingPeerDecision.unknown => LucideIcons.circleHelp,
        PairingPeerDecision.waiting => LucideIcons.hourglass,
        PairingPeerDecision.confirmed => LucideIcons.circleCheck,
      };

  static String pairingPeerDecisionLabel(PairingPeerDecision decision) {
    final strings = AppStrings.current;
    return switch (decision) {
      PairingPeerDecision.unknown => strings.pairingDecisionUnknown,
      PairingPeerDecision.waiting => strings.pairingDecisionWaiting,
      PairingPeerDecision.confirmed => strings.pairingDecisionConfirmed,
    };
  }

  static AppStatusTone pairingPeerDecisionTone(PairingPeerDecision decision) =>
      switch (decision) {
        PairingPeerDecision.confirmed => AppStatusTone.success,
        PairingPeerDecision.unknown ||
        PairingPeerDecision.waiting => AppStatusTone.muted,
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
