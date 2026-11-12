import 'package:intl/intl.dart';

import '../../../l10n/app_localizations.dart';

/// Offered values, change direction and exact binary formatting of the
/// history limits.
abstract final class SettingsLimits {
  static const int mebibyte = 1024 * 1024;
  static const int gibibyte = 1024 * mebibyte;

  /// Days; zero keeps history forever.
  static const List<int> retentionPresets = [7, 30, 90, 365, 0];

  static const List<int> storageQuotaPresets = [
    1 * gibibyte,
    5 * gibibyte,
    10 * gibibyte,
    25 * gibibyte,
    50 * gibibyte,
  ];

  static const List<int> maxFileSizePresets = [
    1 * mebibyte,
    4 * mebibyte,
    16 * mebibyte,
    64 * mebibyte,
    256 * mebibyte,
    1 * gibibyte,
    4 * gibibyte,
    20 * gibibyte,
  ];

  /// Retention stops from the strictest to "keep forever", including a stored
  /// value that is not a preset so the real value is always shown.
  static List<int> retentionStops(int stored) =>
      _stops(retentionPresets, stored, (days) => days == 0 ? null : days);

  /// Size stops from the smallest to the largest, including [stored].
  static List<int> sizeStops(List<int> presets, int stored) =>
      _stops(presets, stored, (bytes) => bytes);

  /// A stricter retention deletes unpinned items older than the new cut-off.
  static bool retentionTightens({required int from, required int to}) =>
      to != 0 && (from == 0 || to < from);

  /// A smaller quota evicts the oldest unpinned items until usage fits.
  static bool storageQuotaTightens({required int from, required int to}) =>
      to < from;

  static String retentionLabel(AppLocalizations l10n, int days) => days == 0
      ? l10n.settingsRetentionForever
      : l10n.settingsRetentionDays(days);

  /// A limit in the largest binary unit that shows it exactly to two
  /// decimals, otherwise in mebibytes. Measured sizes use `formatBytes`.
  static String limitLabel(AppLocalizations l10n, int bytes) {
    final number = NumberFormat('#,##0.##', l10n.localeName);
    if (bytes >= gibibyte && (bytes * 100) % gibibyte == 0) {
      return l10n.sharedSizeGibibytes(number.format(bytes / gibibyte));
    }
    return l10n.sharedSizeMebibytes(number.format(bytes / mebibyte));
  }

  /// Orders by looseness; a null key is unlimited and sorts last.
  static List<int> _stops(
    List<int> presets,
    int stored,
    int? Function(int value) key,
  ) {
    final values = {...presets, stored}.toList();
    values.sort((a, b) {
      final x = key(a);
      final y = key(b);
      if (x == null || y == null) return x == y ? 0 : (x == null ? 1 : -1);
      return x.compareTo(y);
    });
    return List.unmodifiable(values);
  }
}
