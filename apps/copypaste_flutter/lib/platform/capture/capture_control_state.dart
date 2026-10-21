/// Authoritative capture state shared by application and native controls.
enum CaptureControlState {
  unavailable,
  paused,
  running,
  stopped;

  bool get canResume => this == paused || this == stopped;

  String get label => switch (this) {
    unavailable => 'Unavailable',
    paused => 'Paused',
    running => 'Capturing',
    stopped => 'Not running',
  };

  String get actionLabel => switch (this) {
    unavailable => 'Capture unavailable',
    paused => 'Resume capture',
    running => 'Pause capture',
    stopped => 'Resume capture (not running)',
  };
}
