import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../l10n/l10n.dart';
import '../../settings/controller/settings_navigation_state.dart';
import '../../settings/view/settings_search_target.dart';
import '../controller/snippets_controller.dart';
import 'snippet_transfer_dialogs.dart';
import 'snippets_settings_view.dart';

/// Rows of the Snippets section that settings search can reveal.
class SnippetsSettingsTargets {
  static const libraryId = 'result-snippets-library';
  static const transferId = 'result-snippets-transfer';
  static const recordPastedId = 'result-snippets-record-pasted';
  static const requireAuthId = 'result-snippets-require-auth';

  final library = GlobalKey(debugLabel: libraryId);
  final transfer = GlobalKey(debugLabel: transferId);
  final recordPasted = GlobalKey(debugLabel: recordPastedId);
  final requireAuth = GlobalKey(debugLabel: requireAuthId);

  List<SettingsSearchTarget> searchTargets(BuildContext context) {
    final l10n = context.l10n;
    SettingsSearchTarget target(
      String id,
      GlobalKey key,
      String label,
      String description,
      String keywords,
    ) => SettingsSearchTarget(
      id: id,
      section: SettingsSectionId.snippets,
      label: label,
      description: description,
      keywords: keywords,
      targetKey: key,
    );
    return [
      target(
        libraryId,
        library,
        l10n.snippetsLibraryTitle,
        l10n.snippetsLibraryDescription,
        l10n.snippetsLibraryKeywords,
      ),
      target(
        transferId,
        transfer,
        l10n.snippetsTransferTitle,
        l10n.snippetsTransferDescription,
        l10n.snippetsTransferKeywords,
      ),
      target(
        recordPastedId,
        recordPasted,
        l10n.snippetsRecordPastedTitle,
        l10n.snippetsRecordPastedDescription,
        l10n.snippetsRecordPastedKeywords,
      ),
      target(
        requireAuthId,
        requireAuth,
        l10n.snippetsRequireAuthTitle,
        l10n.snippetsRequireAuthDescription,
        l10n.snippetsRequireAuthKeywords,
      ),
    ];
  }
}

/// The Snippets settings contribution.
SettingsFeatureSection snippetsSettingsSection(
  SnippetsController controller, {
  SnippetTransferFiles files = const SnippetTransferFiles(),
}) {
  final targets = SnippetsSettingsTargets();
  return SettingsFeatureSection(
    section: SettingsSectionId.snippets,
    builder: (context, scope) => SnippetsSettingsView(
      controller: controller,
      targets: targets,
      scope: scope,
      files: files,
    ),
    searchTargets: targets.searchTargets,
  );
}
