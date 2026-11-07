import '../l10n/l10n.dart';

/// Shared file-size presentation used by History and modules.
String formatBytes(int bytes) {
  final strings = AppStrings.current;
  if (bytes < 1024) return strings.sharedSizeBytes('$bytes');
  if (bytes < 1024 * 1024) {
    return strings.sharedSizeKilobytes((bytes / 1024).toStringAsFixed(1));
  }
  return strings.sharedSizeMegabytes(
    (bytes / (1024 * 1024)).toStringAsFixed(1),
  );
}
