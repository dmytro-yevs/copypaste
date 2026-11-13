import 'package:flutter/foundation.dart';

/// What a connected AI tool may do; no scope allows deletion or export.
enum AiToolScope { readHistory, readSnippets, writeClipboard, organize }

/// Scopes a new connection starts with: reading only.
const defaultAiToolScopes = {AiToolScope.readHistory, AiToolScope.readSnippets};

/// How far back unpinned history is visible to a connection.
enum AiToolLookback {
  day('day'),
  week('week'),
  month('month'),
  all('all');

  const AiToolLookback(this.wireName);

  final String wireName;
}

/// Content kinds the exposure policy can include.
enum AiToolContentKind {
  text('text'),
  link('link'),
  richText('rich_text'),
  image('image'),
  fileMetadata('file_metadata');

  const AiToolContentKind(this.wireName);

  final String wireName;
}

/// Clients the Connect dialog has configuration for.
enum AiToolClient {
  claudeDesktop('claude_desktop', 'Claude Desktop'),
  claudeCode('claude_code', 'Claude Code'),
  codex('codex', 'Codex'),
  cursor('cursor', 'Cursor'),
  vscode('vscode', 'VS Code'),
  other('other', null);

  const AiToolClient(this.wireName, this.brandName);

  final String wireName;

  /// The product name, which is not translated; `null` for [other].
  final String? brandName;
}

/// The synced AI exposure policy, one per account.
@immutable
class AiToolsPolicy {
  const AiToolsPolicy({
    this.includeOtherDevices = true,
    this.excludedDeviceIds = const [],
    this.contentKinds = const {
      AiToolContentKind.text,
      AiToolContentKind.link,
      AiToolContentKind.richText,
      AiToolContentKind.fileMetadata,
    },
    this.excludedSourceApps = const [],
    this.defaultLookback = AiToolLookback.week,
  });

  final bool includeOtherDevices;
  final List<String> excludedDeviceIds;
  final Set<AiToolContentKind> contentKinds;

  /// Source application identifiers (bundle or package ids).
  final List<String> excludedSourceApps;
  final AiToolLookback defaultLookback;
}

/// One client connection; grants of every desktop are listed.
@immutable
class AiToolGrant {
  const AiToolGrant({
    required this.id,
    required this.ownerDeviceId,
    required this.name,
    required this.clientName,
    required this.scopes,
    required this.lookback,
    required this.createdAt,
    this.ownerDeviceName,
    this.clientExecutable,
    this.lastUsedDay,
    this.revoked = false,
    this.reissueRequired = false,
  });

  final String id;
  final String ownerDeviceId;
  final String? ownerDeviceName;
  final String name;

  /// Reported by the client; informational and untrusted.
  final String clientName;
  final String? clientExecutable;
  final Set<AiToolScope> scopes;
  final AiToolLookback lookback;
  final DateTime createdAt;

  /// The UTC day of the last authorized call; synced by the day.
  final DateTime? lastUsedDay;
  final bool revoked;

  /// This device owns the grant but lost its verifier (after a restore).
  final bool reissueRequired;
}

/// A copyable configuration for one client, with the credential embedded.
@immutable
class AiClientConfig {
  const AiClientConfig({
    required this.client,
    required this.label,
    required this.language,
    required this.text,
  });

  final String client;
  final String label;
  final String language;
  final String text;
}

/// A new connection; the credential exists only in this value.
@immutable
class AiToolGrantCreated {
  const AiToolGrantCreated({required this.grant, required this.configs});

  final AiToolGrant grant;
  final List<AiClientConfig> configs;
}

enum AiApprovalKind { access, volume }

/// A request that waits for a decision at this desktop.
@immutable
class AiApprovalRequest {
  const AiApprovalRequest({
    required this.requestId,
    required this.kind,
    required this.clientName,
    required this.expiresAt,
    this.clientVersion,
    this.executable,
    this.grantName,
  });

  final String requestId;
  final AiApprovalKind kind;

  /// Reported by the client; shown as reported, never trusted.
  final String clientName;
  final String? clientVersion;
  final String? executable;
  final String? grantName;
  final DateTime expiresAt;

  /// The name the prompts show: the grant's, else the reported one.
  String get displayName => grantName ?? clientName;
}

/// The person's answer to an approval prompt.
@immutable
class AiApprovalDecision {
  const AiApprovalDecision.deny()
    : allowed = false,
      scopes = const {},
      lookback = AiToolLookback.week;

  const AiApprovalDecision.allow({
    this.scopes = const {},
    this.lookback = AiToolLookback.week,
  }) : allowed = true;

  final bool allowed;
  final Set<AiToolScope> scopes;
  final AiToolLookback lookback;
}

/// One access-log entry; it never holds content or query text.
@immutable
class AiAccessEntry {
  const AiAccessEntry({
    required this.at,
    required this.tool,
    required this.resultCount,
    this.error,
  });

  final DateTime at;
  final String tool;
  final int resultCount;
  final String? error;
}

/// The serving state of this desktop.
@immutable
class AiToolsStatus {
  const AiToolsStatus({required this.enabled, this.contributingDevices = 0});

  final bool enabled;
  final int contributingDevices;
}

/// A device or source application the policy can exclude.
@immutable
class AiToolsOption {
  const AiToolsOption({required this.id, required this.label, this.iconId});

  final String id;
  final String label;
  final String? iconId;
}

/// Devices and source applications present in history.
@immutable
class AiToolsExposureOptions {
  const AiToolsExposureOptions({
    this.devices = const [],
    this.sourceApps = const [],
  });

  final List<AiToolsOption> devices;
  final List<AiToolsOption> sourceApps;
}

/// A runtime failure with its error code and user-facing message.
class AiToolsException implements Exception {
  const AiToolsException(this.code, this.message);

  final String code;
  final String message;

  @override
  String toString() => 'AiToolsException($code)';
}
