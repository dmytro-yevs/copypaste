import 'package:copypaste_flutter/l10n/l10n.dart';

/// Authoritative capture state shared by application and native controls.
enum CaptureControlState {
  unavailable,
  paused,
  running,
  stopped;

  bool get canResume => this == paused || this == stopped;

  String get label => switch (this) {
    unavailable => AppStrings.current.platformCaptureStateUnavailable,
    paused => AppStrings.current.platformCaptureStatePaused,
    running => AppStrings.current.platformCaptureStateRunning,
    stopped => AppStrings.current.platformCaptureStateStopped,
  };

  String get actionLabel => switch (this) {
    unavailable => AppStrings.current.platformCaptureActionUnavailable,
    paused => AppStrings.current.platformCaptureActionResume,
    running => AppStrings.current.platformCaptureActionPause,
    stopped => AppStrings.current.platformCaptureActionResumeStopped,
  };
}
