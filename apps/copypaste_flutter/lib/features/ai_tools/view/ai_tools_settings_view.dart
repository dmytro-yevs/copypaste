import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../../shared/system_date_time.dart';
import '../../settings/controller/settings_navigation_state.dart';
import '../../settings/view/settings_search_target.dart';
import '../../settings/view/settings_section.dart';
import '../controller/ai_tools_controller.dart';
import '../models/ai_tools_models.dart';
import '../presentation/ai_tools_labels.dart';
import 'ai_connect_dialog.dart';
import 'ai_grant_dialogs.dart';
import 'ai_policy_editor.dart';
import 'ai_tools_settings_targets.dart';

/// Settings → AI tools as a settings screen contribution.
SettingsFeatureSection aiToolsSettingsSection(AiToolsController controller) {
  final targets = AiToolsSettingsTargets(serving: controller.serving);
  return SettingsFeatureSection(
    section: SettingsSectionId.aiTools,
    builder: (context, scope) => AiToolsSettingsView(
      controller: controller,
      targets: targets,
      scope: scope,
    ),
    searchTargets: targets.searchTargets,
  );
}

/// The local toggle (desktop), connected tools of every desktop, the
/// Connect flow (desktop) and the synced exposure policy. Android shows
/// the same sections minus the toggle and Connect. It loads while shown.
class AiToolsSettingsView extends StatefulWidget {
  const AiToolsSettingsView({
    super.key,
    required this.controller,
    required this.targets,
    this.scope = const SettingsSectionScope(highlightedTargetId: null),
  });

  final AiToolsController controller;
  final AiToolsSettingsTargets targets;
  final SettingsSectionScope scope;

  @override
  State<AiToolsSettingsView> createState() => _AiToolsSettingsViewState();
}

class _AiToolsSettingsViewState extends State<AiToolsSettingsView> {
  AiToolsController get _controller => widget.controller;

  @override
  void initState() {
    super.initState();
    _controller.attach();
  }

  @override
  void dispose() {
    _controller.detach();
    super.dispose();
  }

  bool _highlighted(String id) => widget.scope.isHighlighted(id);

  void _run(Future<bool> Function() change) => unawaited(change());

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: _controller,
    builder: (context, _) {
      final l10n = context.l10n;
      switch (_controller.status) {
        case AiToolsLoadStatus.idle || AiToolsLoadStatus.loading:
          return const StateView.loading();
        case AiToolsLoadStatus.failed:
          return StateView.error(
            title: l10n.aiToolsLoadFailed,
            message: _controller.loadError,
            actionLabel: l10n.commonTryAgain,
            onAction: () => unawaited(_controller.retry()),
          );
        case AiToolsLoadStatus.ready:
          break;
      }
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (_controller.actionError case final error?) ...[
            Alert.destructive(
              leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
              title: Text(l10n.appActionFailed),
              content: Text(error),
            ),
            const Gap(AppSpacing.lg),
          ],
          SettingsGroup(
            children: [
              if (_controller.serving) _enabledRow(context),
              Alert(
                leading: const AppIconFrame(child: Icon(LucideIcons.shield)),
                title: Text(l10n.aiToolsThreatModelTitle),
                content: Text(l10n.aiToolsThreatModel),
              ),
            ],
          ),
          const Gap(AppSpacing.xxl),
          SettingsGroup(children: [_grantsRow(context)]),
          const Gap(AppSpacing.xxl),
          AiPolicyEditor(
            controller: _controller,
            targets: widget.targets,
            scope: widget.scope,
          ),
        ],
      );
    },
  );

  Widget _enabledRow(BuildContext context) {
    final l10n = context.l10n;
    final status = _controller.serverStatus;
    return SettingRow(
      key: widget.targets.enabled,
      highlighted: _highlighted(AiToolsSettingsTargets.enabledId),
      title: l10n.aiToolsEnableTitle,
      description: l10n.aiToolsEnableDescription,
      status: Text(
        status.enabled
            ? l10n.aiToolsStatusOn(status.contributingDevices)
            : l10n.aiToolsStatusOff,
      ).xSmall().muted(),
      trailing: Switch(
        key: const ValueKey('ai-tools-enabled'),
        value: status.enabled,
        onChanged: _controller.busy
            ? null
            : (value) => _run(() => _controller.setEnabled(value)),
      ),
    );
  }

  Widget _grantsRow(BuildContext context) {
    final l10n = context.l10n;
    final grants = _controller.grants;
    final serving = _controller.serving;
    final connect = serving
        ? Button.secondary(
            key: const ValueKey('ai-tools-connect'),
            style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
            onPressed: _controller.busy
                ? null
                : () => unawaited(
                    showAiConnectDialog(context, controller: _controller),
                  ),
            leading: const AppIconFrame(child: Icon(LucideIcons.plus)),
            child: Text(l10n.aiToolsConnect),
          )
        : null;
    final desktops = {
      for (final grant in grants)
        if (!grant.revoked) grant.ownerDeviceName ?? l10n.aiToolsUnknownDevice,
    };
    return SettingRow(
      key: widget.targets.grants,
      highlighted: _highlighted(AiToolsSettingsTargets.grantsId),
      title: l10n.aiToolsGrantsTitle,
      description: l10n.aiToolsGrantsDescription,
      status: !serving && desktops.isNotEmpty
          ? Text(
              l10n.aiToolsServedThrough(desktops.join(', ')),
            ).xSmall().muted()
          : null,
      trailing: grants.isEmpty ? null : connect,
      footer: grants.isEmpty
          ? StateView.empty(
              compact: true,
              icon: LucideIcons.bot,
              title: serving
                  ? l10n.aiToolsEmptyTitle
                  : l10n.aiToolsEmptyServedTitle,
              message: serving
                  ? l10n.aiToolsEmptyMessage
                  : l10n.aiToolsEmptyServedMessage,
              actionLabel: serving ? l10n.aiToolsConnect : null,
              onAction: serving
                  ? () => unawaited(
                      showAiConnectDialog(context, controller: _controller),
                    )
                  : null,
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                for (final (index, grant) in grants.indexed) ...[
                  if (index > 0) const Divider(),
                  _grantRow(context, grant),
                ],
              ],
            ),
    );
  }

  String _grantStatus(BuildContext context, AiToolGrant grant) {
    final l10n = context.l10n;
    final owner = grant.ownerDeviceName ?? l10n.aiToolsUnknownDevice;
    final local = _controller.isLocal(grant);
    if (grant.revoked) {
      return local
          ? l10n.aiToolsGrantRevoked
          : l10n.aiToolsGrantRevokedRemote(owner);
    }
    if (local && grant.reissueRequired) return l10n.aiToolsGrantReissue;
    if (local && _controller.serving && !_controller.serverStatus.enabled) {
      return l10n.aiToolsGrantPaused;
    }
    final used = grant.lastUsedDay;
    if (used == null) return l10n.aiToolsGrantWaiting;
    return l10n.aiToolsGrantUsed(
      formatSystemDate(context, DateTime(used.year, used.month, used.day)),
    );
  }

  Widget _grantRow(BuildContext context, AiToolGrant grant) {
    final l10n = context.l10n;
    final busy = _controller.busy;
    final local = _controller.isLocal(grant);
    final details = [
      l10n.aiToolsLookback(grant.lookback),
      if (!local)
        l10n.aiToolsGrantOwner(
          grant.ownerDeviceName ?? l10n.aiToolsUnknownDevice,
        ),
      _grantStatus(context, grant),
    ].join(' · ');
    return Padding(
      key: ValueKey('ai-grant-${grant.id}'),
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.xs),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  grant.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ).small().medium(),
                const Gap(AppSpacing.xxs),
                Wrap(
                  spacing: AppSpacing.xs,
                  runSpacing: AppSpacing.xs,
                  children: [
                    for (final scope in AiToolScope.values)
                      if (grant.scopes.contains(scope))
                        Chip(child: Text(l10n.aiToolsScope(scope))),
                  ],
                ),
                const Gap(AppSpacing.xxs),
                Text(details).xSmall().muted(),
              ],
            ),
          ),
          _iconButton(
            l10n.aiToolsEdit,
            LucideIcons.pencil,
            busy || grant.revoked
                ? null
                : () => unawaited(
                    showAiGrantEditDialog(
                      context,
                      controller: _controller,
                      grant: grant,
                    ),
                  ),
          ),
          if (local)
            _iconButton(
              l10n.aiToolsActivity,
              LucideIcons.history,
              () => unawaited(
                showAiGrantActivityDialog(
                  context,
                  controller: _controller,
                  grant: grant,
                ),
              ),
            ),
          if (!grant.revoked)
            _iconButton(
              l10n.aiToolsRevoke,
              LucideIcons.ban,
              busy ? null : () => unawaited(_confirmRevoke(grant)),
            ),
          _iconButton(
            l10n.commonDelete,
            LucideIcons.trash2,
            busy ? null : () => unawaited(_confirmDelete(grant)),
          ),
        ],
      ),
    );
  }

  Future<void> _confirmRevoke(AiToolGrant grant) =>
      AppOverlays.showActionDialog(
        context,
        icon: LucideIcons.ban,
        title: Text(context.l10n.aiToolsRevokeTitle(grant.name)),
        content: Text(context.l10n.aiToolsRevokeMessage),
        actionLabel: context.l10n.aiToolsRevoke,
        failureMessage: () => context.l10n.aiToolsChangeFailed,
        onConfirm: () async {
          if (!await _controller.revokeGrant(grant)) {
            throw StateError('grant not revoked');
          }
          return true;
        },
      );

  Future<void> _confirmDelete(AiToolGrant grant) =>
      AppOverlays.showActionDialog(
        context,
        icon: LucideIcons.trash2,
        title: Text(context.l10n.aiToolsDeleteTitle(grant.name)),
        content: Text(context.l10n.aiToolsDeleteMessage),
        actionLabel: context.l10n.commonDelete,
        failureMessage: () => context.l10n.aiToolsChangeFailed,
        onConfirm: () async {
          if (!await _controller.deleteGrant(grant)) {
            throw StateError('grant not deleted');
          }
          return true;
        },
      );

  Widget _iconButton(String label, IconData icon, VoidCallback? onPressed) =>
      Tooltip(
        tooltip: (context) => TooltipContainer(child: Text(label)),
        child: Button.ghost(
          style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
          onPressed: onPressed,
          child: Semantics(
            label: label,
            child: AppIconFrame(child: Icon(icon)),
          ),
        ),
      );
}
