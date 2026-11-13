import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../controller/snippet_editor_controller.dart';
import '../models/snippet_models.dart';

extension SnippetMessages on AppLocalizations {
  /// Lint codes arrive in snake_case; messages select on camelCase.
  String snippetLint(TemplateLint lint) => snippetsLint(
    lint.code.replaceAllMapped(
      RegExp('_([a-z])'),
      (match) => match[1]!.toUpperCase(),
    ),
    lint.detail ?? '',
  );

  String snippetSaveBlock(SnippetSaveBlock block, {String owner = ''}) =>
      switch (block) {
        SnippetSaveBlock.emptyBody => snippetsSaveBlockEmpty,
        SnippetSaveBlock.bodyTooLarge => snippetsSaveBlockTooLarge,
        SnippetSaveBlock.titleTooLong => snippetsSaveBlockTitle(
          SnippetLimits.maxTitle,
        ),
        SnippetSaveBlock.abbreviationTooLong => snippetsAbbreviationTooLong(
          SnippetLimits.maxAbbreviationGraphemes,
        ),
        SnippetSaveBlock.abbreviationWhitespace =>
          snippetsAbbreviationWhitespace,
        SnippetSaveBlock.abbreviationTaken => snippetsAbbreviationTaken(owner),
      };

  /// The list title of [snippet]; concealed ones never show their body.
  String snippetTitle(SnippetSummary snippet) => snippet.title.trim().isNotEmpty
      ? snippet.title.trim()
      : snippet.concealed
      ? snippetsConcealedTitle
      : snippet.displayTitle ?? snippetsUntitled;
}

/// Icons and accents of collections.
abstract final class SnippetCollectionStyle {
  static IconData icon(SnippetCollectionIcon icon) => switch (icon) {
    SnippetCollectionIcon.folder => LucideIcons.folder,
    SnippetCollectionIcon.briefcase => LucideIcons.briefcase,
    SnippetCollectionIcon.house => LucideIcons.house,
    SnippetCollectionIcon.code => LucideIcons.code,
    SnippetCollectionIcon.mail => LucideIcons.mail,
    SnippetCollectionIcon.messageSquare => LucideIcons.messageSquare,
    SnippetCollectionIcon.star => LucideIcons.star,
    SnippetCollectionIcon.bookmark => LucideIcons.bookmark,
    SnippetCollectionIcon.user => LucideIcons.user,
    SnippetCollectionIcon.heart => LucideIcons.heart,
  };

  static Color accent(BuildContext context, SnippetCollectionAccent accent) =>
      switch (accent) {
        SnippetCollectionAccent.neutral => AppTheme.statusColor(
          context,
          AppStatusTone.muted,
        ),
        SnippetCollectionAccent.blue => AppTheme.statusColor(
          context,
          AppStatusTone.info,
        ),
        SnippetCollectionAccent.green => AppTheme.statusColor(
          context,
          AppStatusTone.success,
        ),
        SnippetCollectionAccent.amber => AppTheme.statusColor(
          context,
          AppStatusTone.warning,
        ),
        SnippetCollectionAccent.red => AppTheme.statusColor(
          context,
          AppStatusTone.error,
        ),
      };

  static String accentLabel(
    AppLocalizations l10n,
    SnippetCollectionAccent accent,
  ) => l10n.snippetsCollectionAccent(accent.name);
}
