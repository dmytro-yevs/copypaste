import '../../../l10n/app_localizations.dart';
import '../../../shared/format_bytes.dart';
import 'settings_models.dart';

/// What to tell the user after a data action completed.
abstract final class SettingsOutcomes {
  /// Clips left out because they are not text are expected and not named;
  /// unreadable text clips are.
  static String exported(AppLocalizations l10n, TextExportResult result) {
    final exported = l10n.settingsExportedTextClips(result.exported);
    if (result.skippedUndecryptable == 0) return exported;
    return l10n.settingsOutcomeDetail(
      exported,
      l10n.settingsExportUnreadableClips(result.skippedUndecryptable),
    );
  }

  static String backedUp(AppLocalizations l10n, BackupResult result) =>
      l10n.settingsBackupDoneMessage(formatBytes(result.sizeBytes));
}
