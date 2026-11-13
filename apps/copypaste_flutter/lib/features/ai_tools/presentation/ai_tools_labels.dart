import '../../../l10n/l10n.dart';
import '../models/ai_tools_models.dart';

/// Localized names of AI tool scopes, windows, kinds and clients.
extension AiToolsLabels on AppLocalizations {
  String aiToolsScope(AiToolScope scope) => switch (scope) {
    AiToolScope.readHistory => aiToolsScopeReadHistory,
    AiToolScope.readSnippets => aiToolsScopeReadSnippets,
    AiToolScope.writeClipboard => aiToolsScopeWriteClipboard,
    AiToolScope.organize => aiToolsScopeOrganize,
  };

  String aiToolsLookback(AiToolLookback lookback) => switch (lookback) {
    AiToolLookback.day => aiToolsLookbackDay,
    AiToolLookback.week => aiToolsLookbackWeek,
    AiToolLookback.month => aiToolsLookbackMonth,
    AiToolLookback.all => aiToolsLookbackAll,
  };

  String aiToolsContentKind(AiToolContentKind kind) => switch (kind) {
    AiToolContentKind.text => aiToolsKindText,
    AiToolContentKind.link => aiToolsKindLink,
    AiToolContentKind.richText => aiToolsKindRichText,
    AiToolContentKind.image => aiToolsKindImage,
    AiToolContentKind.fileMetadata => aiToolsKindFileMetadata,
  };

  String aiToolsClient(AiToolClient client) =>
      client.brandName ?? aiToolsClientOther;
}
