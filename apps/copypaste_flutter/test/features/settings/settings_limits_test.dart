import 'package:copypaste_flutter/features/settings/models/desktop_presence.dart';
import 'package:copypaste_flutter/features/settings/models/settings_limits.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/models/settings_outcomes.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:copypaste_flutter/shared/format_bytes.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final en = lookupAppLocalizations(const Locale('en'));
  final de = lookupAppLocalizations(const Locale('de'));
  const gib = SettingsLimits.gibibyte;
  const mib = SettingsLimits.mebibyte;

  test('only a stricter retention or a smaller quota deletes history', () {
    // Forever is the loosest retention; any limit after it is stricter.
    expect(SettingsLimits.retentionTightens(from: 0, to: 365), isTrue);
    expect(SettingsLimits.retentionTightens(from: 90, to: 30), isTrue);
    expect(SettingsLimits.retentionTightens(from: 30, to: 90), isFalse);
    expect(SettingsLimits.retentionTightens(from: 7, to: 0), isFalse);
    expect(SettingsLimits.retentionTightens(from: 30, to: 30), isFalse);
    expect(
      SettingsLimits.storageQuotaTightens(from: 10 * gib, to: 5 * gib),
      isTrue,
    );
    expect(
      SettingsLimits.storageQuotaTightens(from: 5 * gib, to: 10 * gib),
      isFalse,
    );
  });

  test('stops run from strict to loose and keep an off-preset value', () {
    expect(SettingsLimits.retentionStops(0), [7, 30, 90, 365, 0]);
    expect(SettingsLimits.retentionStops(14), [7, 14, 30, 90, 365, 0]);
    expect(
      SettingsLimits.sizeStops(
        SettingsLimits.storageQuotaPresets,
        (1.5 * gib).round(),
      ).take(3),
      [1 * gib, (1.5 * gib).round(), 5 * gib],
    );
  });

  test('sizes use binary units with locale decimals', () {
    expect(SettingsLimits.limitLabel(en, 10 * gib), '10 GiB');
    expect(SettingsLimits.limitLabel(en, (1.5 * gib).round()), '1.5 GiB');
    expect(SettingsLimits.limitLabel(de, (1.5 * gib).round()), '1,5 GiB');
    expect(SettingsLimits.limitLabel(en, 256 * mib), '256 MiB');
    expect(SettingsLimits.limitLabel(en, 50 * mib), '50 MiB');
    expect(formatBytes((12.4 * mib).round()), '12.4 MiB');
    expect(formatBytes(2048), '2.0 KiB');
    expect(formatBytes(512), '512 B');
    expect(formatBytes(3 * gib ~/ 2), '1.5 GiB');
    AppStrings.useLocale(const Locale('fr'));
    addTearDown(() => AppStrings.useLocale(AppLocalization.fallback));
    expect(formatBytes(1536), '1,5 Kio');
    expect(SettingsLimits.retentionLabel(en, 0), 'Keep forever');
  });

  test('outcomes name unreadable clips but not skipped non-text clips', () {
    expect(
      SettingsOutcomes.exported(
        en,
        const TextExportResult(
          exported: 214,
          skippedNonText: 9,
          skippedUndecryptable: 0,
        ),
      ),
      'Exported 214 text clips',
    );
    expect(
      SettingsOutcomes.exported(
        en,
        const TextExportResult(
          exported: 214,
          skippedNonText: 9,
          skippedUndecryptable: 3,
        ),
      ),
      'Exported 214 text clips · 3 could not be read',
    );
    expect(
      SettingsOutcomes.backedUp(
        en,
        BackupResult(sizeBytes: (12.4 * mib).round()),
      ),
      'Encrypted backup saved · 12.4 MiB',
    );
  });

  test('hiding the last icon names the registered shortcut or relaunch', () {
    expect(
      DesktopPresence.hidesLastEntryPoint(
        turningOn: false,
        otherVisible: false,
      ),
      isTrue,
    );
    expect(
      DesktopPresence.hidesLastEntryPoint(turningOn: false, otherVisible: true),
      isFalse,
    );
    expect(
      DesktopPresence.hidesLastEntryPoint(turningOn: true, otherVisible: false),
      isFalse,
    );
    expect(
      DesktopPresence.hiddenEverywhereMessage(en),
      contains('start CopyPaste again'),
    );
    const shortcut = DesktopShortcut(
      key: PhysicalKeyboardKey.keyC,
      modifiers: [
        DesktopShortcutModifier.control,
        DesktopShortcutModifier.shift,
      ],
    );
    expect(
      DesktopPresence.hiddenEverywhereMessage(en, registeredShortcut: shortcut),
      contains('press Ctrl+Shift+C or start CopyPaste again'),
    );
    expect(
      DesktopPresence.hiddenEverywhereMessage(
        en,
        registeredShortcut: shortcut,
        registeredShortcutDescription: 'Super+V',
      ),
      contains('press Super+V'),
    );
  });
}
