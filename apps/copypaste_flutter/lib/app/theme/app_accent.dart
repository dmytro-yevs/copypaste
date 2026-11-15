import 'package:shadcn_flutter/shadcn_flutter.dart' show Color;

import 'app_theme.dart';

/// The accent the application UI highlights with: the system's, the
/// application's standard blue, or one of the presets.
enum AppAccent {
  system(null),
  standard(AppTheme.navigationAccent),
  purple(Color(0xFF8E5AF7)),
  pink(Color(0xFFE5508F)),
  red(Color(0xFFE5484D)),
  orange(Color(0xFFF76B15)),
  yellow(Color(0xFFE2A400)),
  green(Color(0xFF30A46C)),
  teal(Color(0xFF12A594)),
  graphite(Color(0xFF8B8D98));

  const AppAccent(this.color);

  /// Null for [system], which the platform resolves.
  final Color? color;
}
