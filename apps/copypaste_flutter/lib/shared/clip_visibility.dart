/// How a clip may be presented.
enum ClipPresentation {
  /// Content may be shown.
  full,

  /// Only a masked preview with type and length hints may be shown.
  masked,

  /// The clip must not be listed: it expired and awaits its purge.
  hidden,
}

/// The single Dart answer to whether a clip is secret or expired, derived from
/// its privacy flag and runtime annotations.
///
/// Every surface (History, Quick Paste, tray, drawers) uses this instead of
/// reading the flags itself, so the rules stay identical everywhere.
class ClipVisibility {
  const ClipVisibility({
    required this.secret,
    this.ruleSecret = false,
    this.expiresAt,
  });

  /// The authenticated secret flag captured with the clip.
  final bool secret;

  /// A local display mask applied by a content rule.
  final bool ruleSecret;
  final DateTime? expiresAt;

  bool get masked => secret || ruleSecret;

  bool isExpired(DateTime now) {
    final expiresAt = this.expiresAt;
    return expiresAt != null && !expiresAt.isAfter(now);
  }

  /// [maskedMode] masks every clip, for example while a password field is
  /// the paste target.
  ClipPresentation presentationAt(DateTime now, {bool maskedMode = false}) {
    if (isExpired(now)) return ClipPresentation.hidden;
    if (masked || maskedMode) return ClipPresentation.masked;
    return ClipPresentation.full;
  }

  /// Time left before an expiring clip is deleted, or `null` without expiry.
  Duration? remainingAt(DateTime now) {
    final expiresAt = this.expiresAt;
    if (expiresAt == null) return null;
    final remaining = expiresAt.difference(now);
    return remaining.isNegative ? Duration.zero : remaining;
  }
}
