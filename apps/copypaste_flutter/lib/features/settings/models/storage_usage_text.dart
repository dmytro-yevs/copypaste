import '../../../l10n/app_localizations.dart';
import '../../../shared/format_bytes.dart';
import 'settings_limits.dart';
import 'settings_models.dart';

/// Words for storage used against the quota. Measured sizes use
/// [formatBytes]; the quota is named exactly, as the quota scale names it.
abstract final class StorageUsageText {
  /// "2.1 of 5 GiB": the used amount drops its unit when it reads in the
  /// quota's unit, otherwise both carry one.
  static String short(AppLocalizations l10n, StorageUsage usage) {
    if (!usage.hasQuota) return formatBytes(usage.usedBytes);
    final quota = SettingsLimits.limitLabel(l10n, usage.quotaBytes);
    final unit = _sharedUnit(usage.usedBytes, usage.quotaBytes);
    final used = unit == null
        ? formatBytes(usage.usedBytes)
        : formatSizeNumber(usage.usedBytes / unit);
    return l10n.storageUsageShort(used, quota);
  }

  /// "2.1 GiB used of 5 GiB", or "2.1 GiB used" without a quota; usage over
  /// the quota says so in words.
  static String summary(AppLocalizations l10n, StorageUsage usage) {
    final used = formatBytes(usage.usedBytes);
    final amount = usage.hasQuota
        ? l10n.storageUsageOfQuota(
            used,
            SettingsLimits.limitLabel(l10n, usage.quotaBytes),
          )
        : l10n.storageUsageNoQuota(used);
    return usage.overQuota
        ? l10n.storageUsageDetail(amount, l10n.storageUsageOverQuota)
        : amount;
  }

  /// [summary] with the oldest clip's date when there is one.
  static String withOldest(
    AppLocalizations l10n,
    StorageUsage usage,
    String? oldestDate,
  ) {
    final text = summary(l10n, usage);
    return oldestDate == null
        ? text
        : l10n.storageUsageDetail(
            text,
            l10n.storageUsageOldestClip(oldestDate),
          );
  }

  /// The unit both sizes are written in, or null when they differ.
  static int? _sharedUnit(int used, int quota) {
    const mebibyte = SettingsLimits.mebibyte;
    const gibibyte = SettingsLimits.gibibyte;
    final quotaInGibibytes = quota >= gibibyte && (quota * 100) % gibibyte == 0;
    if (quotaInGibibytes) return used >= gibibyte ? gibibyte : null;
    return used >= mebibyte && used < gibibyte ? mebibyte : null;
  }
}
