import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../../shared/system_date_time.dart';
import '../../settings/view/settings_search_target.dart';
import '../../settings/view/settings_section.dart';
import '../controller/content_rules_controller.dart';
import '../models/content_rule_models.dart';
import '../presentation/rule_messages.dart';
import 'content_rules_search_targets.dart';
import 'rule_editor_drawer.dart';
import 'rule_preview_dialog.dart';

/// Settings → Content rules: the Privacy defaults that guard capture, the
/// built-in rules, the user's ordered rules, recently deleted rules and the
/// skip log. It loads while shown and releases on close.
class ContentRulesSettingsView extends StatefulWidget {
  const ContentRulesSettingsView({
    super.key,
    required this.controller,
    required this.targets,
    this.scope = const SettingsSectionScope(highlightedTargetId: null),
    this.onOpenPrivacy,
  });

  final ContentRulesController controller;
  final ContentRulesSettingsTargets targets;
  final SettingsSectionScope scope;
  final VoidCallback? onOpenPrivacy;

  @override
  State<ContentRulesSettingsView> createState() =>
      _ContentRulesSettingsViewState();
}

class _ContentRulesSettingsViewState extends State<ContentRulesSettingsView> {
  final _sortable = <String, SortableData<String>>{};

  ContentRulesController get _controller => widget.controller;

  @override
  void initState() {
    super.initState();
    _controller.attach();
    _controller.editorRequest.addListener(_openRequested);
    // A request to edit a rule waits until the list has loaded.
    _controller.addListener(_openRequested);
    WidgetsBinding.instance.addPostFrameCallback((_) => _openRequested());
  }

  @override
  void dispose() {
    _controller.editorRequest.removeListener(_openRequested);
    _controller.removeListener(_openRequested);
    _controller.detach();
    super.dispose();
  }

  void _openRequested() {
    final request = _controller.editorRequest.value;
    if (request == null || !mounted) return;
    if (request.ruleId != null &&
        _controller.status != ContentRulesStatus.ready) {
      return;
    }
    _controller.editorRequest.value = null;
    final id = request.ruleId;
    final rule = id == null ? null : _controller.rule(id);
    if (id != null && rule == null) return;
    unawaited(_edit(rule, definition: request.definition));
  }

  void _run(Future<bool> Function() change) => unawaited(change());

  Future<void> _edit(ContentRule? rule, {RuleDefinition? definition}) =>
      showRuleEditorDrawer(
        context,
        controller: _controller,
        rule: rule,
        definition: definition,
        onDeleted: _undoToast,
      );

  void _undoToast(String id) {
    if (!mounted) return;
    final l10n = context.l10n;
    AppToast.show(
      context,
      title: l10n.rulesDeletedToast,
      message: l10n.rulesDeletedToastMessage,
      actionLabel: l10n.rulesUndo,
      onAction: () => _run(() => _controller.restore(id)),
    );
  }

  Future<void> _delete(ContentRule rule) async {
    if (await _controller.delete(rule.id)) _undoToast(rule.id);
  }

  Future<void> _preview(ContentRule rule) => showRulePreviewDialog(
    context,
    repository: _controller.repository,
    ruleId: rule.id,
  );

  Future<void> _showSkipLog() async {
    List<RuleSkipEvent> events;
    try {
      events = await _controller.skipLog();
    } on Object {
      events = const [];
    }
    if (!mounted) return;
    await AppOverlays.showDialog<void>(
      context,
      builder: (context) => _SkipLogDialog(events: events.reversed.toList()),
    );
  }

  @override
  Widget build(BuildContext context) {
    final mod = defaultTargetPlatform == TargetPlatform.macOS;
    return CallbackShortcuts(
      bindings: {
        SingleActivator(
          LogicalKeyboardKey.keyN,
          meta: mod,
          control: !mod,
        ): () =>
            unawaited(_edit(null)),
      },
      child: ListenableBuilder(
        listenable: _controller,
        builder: (context, _) {
          final l10n = context.l10n;
          switch (_controller.status) {
            case ContentRulesStatus.idle || ContentRulesStatus.loading:
              return const StateView.loading();
            case ContentRulesStatus.failed:
              return StateView.error(
                title: l10n.rulesLoadFailed,
                message: _controller.errorMessage,
                actionLabel: l10n.commonTryAgain,
                onAction: () => unawaited(_controller.retry()),
              );
            case ContentRulesStatus.ready:
              break;
          }
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (_controller.errorMessage case final error?) ...[
                Alert.destructive(
                  leading: const AppIconFrame(
                    child: Icon(LucideIcons.circleAlert),
                  ),
                  title: Text(l10n.appActionFailed),
                  content: Text(error),
                ),
                const Gap(AppSpacing.lg),
              ],
              for (final conflict in _controller.conflicts) ...[
                _conflict(context, conflict),
                const Gap(AppSpacing.md),
              ],
              _privacyGroup(context),
              const Gap(AppSpacing.xxl),
              _userGroup(context),
              if (_controller.deletedRules.isNotEmpty) ...[
                const Gap(AppSpacing.xxl),
                _deletedGroup(context),
              ],
              const Gap(AppSpacing.xxl),
              SettingsGroup(children: [_skipLogRow(context)]),
            ],
          );
        },
      ),
    );
  }

  Widget _conflict(BuildContext context, RuleConflict conflict) {
    final l10n = context.l10n;
    final name = _controller.rule(conflict.ruleId)?.name ?? l10n.rulesUntitled;
    return Alert(
      key: ValueKey('rule-conflict-${conflict.id}'),
      leading: const AppIconFrame(child: Icon(LucideIcons.gitMerge)),
      content: Text(
        l10n.rulesConflictMessage(
          name,
          conflict.fromDevice ?? l10n.rulesAnotherDevice,
        ),
      ),
      trailing: Button.secondary(
        key: ValueKey('rule-conflict-restore-${conflict.id}'),
        style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
        onPressed: _controller.busy
            ? null
            : () => _run(() => _controller.restoreConflict(conflict.id)),
        child: Text(l10n.rulesRestoreConflict),
      ),
    );
  }

  Widget _privacyGroup(BuildContext context) {
    final l10n = context.l10n;
    return SettingsGroup(
      children: [
        SettingRow(
          key: widget.targets.privacy,
          highlighted: widget.scope.isHighlighted(
            ContentRulesSettingsTargets.privacyId,
          ),
          title: l10n.rulesPrivacyDefaultsTitle,
          description: l10n.rulesPrivacyDefaultsDescription,
          trailing: widget.onOpenPrivacy == null
              ? null
              : Button.secondary(
                  key: const ValueKey('rules-open-privacy'),
                  style: AppTheme.controlButtonStyle(
                    const ButtonStyle.secondary(),
                  ),
                  onPressed: widget.onOpenPrivacy,
                  child: Text(l10n.rulesOpenPrivacy),
                ),
        ),
        for (final rule in _controller.builtins) _row(context, rule, null),
      ],
    );
  }

  Widget _userGroup(BuildContext context) {
    final l10n = context.l10n;
    final rules = _controller.userRules;
    return SortableLayer(
      dropDuration: AppMotion.resolve(context, AppMotion.standard),
      dropCurve: AppMotion.standardCurve,
      child: SettingsGroup(
        children: [
          SettingRow(
            key: widget.targets.rules,
            highlighted: widget.scope.isHighlighted(
              ContentRulesSettingsTargets.rulesId,
            ),
            title: l10n.rulesMyRulesTitle,
            description: l10n.rulesMyRulesDescription,
            trailing: Button.primary(
              key: const ValueKey('rules-new'),
              style: AppTheme.controlButtonStyle(const ButtonStyle.primary()),
              onPressed: () => unawaited(_edit(null)),
              leading: const AppIconFrame(child: Icon(LucideIcons.plus)),
              child: Text(l10n.rulesNewRule),
            ),
          ),
          if (rules.isEmpty)
            StateView.empty(
              key: const ValueKey('rules-empty'),
              compact: true,
              icon: LucideIcons.listFilter,
              title: l10n.rulesEmptyTitle,
              message: l10n.rulesEmptyMessage,
            )
          else
            for (final (index, rule) in rules.indexed)
              _sortableRow(context, rules, index, rule),
        ],
      ),
    );
  }

  Widget _sortableRow(
    BuildContext context,
    List<ContentRule> rules,
    int index,
    ContentRule rule,
  ) {
    bool accepts(SortableData<String> data) => data.data != rule.id;
    int target(String id, {required bool before}) {
      final from = rules.indexWhere((candidate) => candidate.id == id);
      final to = before ? index : index + 1;
      return from < to ? to - 1 : to;
    }

    return Sortable<String>(
      key: ValueKey('rule-sortable-${rule.id}'),
      data: _sortable.putIfAbsent(rule.id, () => SortableData(rule.id)),
      enabled: false,
      canAcceptTop: accepts,
      canAcceptBottom: accepts,
      onAcceptTop: (data) => _run(
        () => _controller.moveTo(data.data, target(data.data, before: true)),
      ),
      onAcceptBottom: (data) => _run(
        () => _controller.moveTo(data.data, target(data.data, before: false)),
      ),
      placeholder: const SizedBox(height: AppControlSize.regular),
      child: _row(context, rule, index),
    );
  }

  /// [index] is the position among user rules; null for a built-in.
  Widget _row(BuildContext context, ContentRule rule, int? index) {
    final l10n = context.l10n;
    final options = _controller.options;
    final definition = rule.definition;
    final mod = defaultTargetPlatform == TargetPlatform.macOS;
    final user = index != null;
    void move(RuleMove direction) =>
        _run(() => _controller.move(rule.id, direction));
    final moves = {
      if (user)
        for (final direction in RuleMove.values)
          if (_controller.canMove(rule.id, direction))
            direction: l10n.rulesMove(direction.name),
    };
    final devices = l10n.ruleDevices(rule.deviceIds, options);
    final summary = definition == null
        ? l10n.rulesUpdateRequired
        : l10n.ruleSummary(definition, options);
    final row = Padding(
      padding: AppTheme.settingsRowPadding,
      child: Row(
        children: [
          if (user)
            SortableDragHandle(
              child: _iconButton(
                l10n.rulesReorder,
                LucideIcons.gripVertical,
                () {},
              ),
            ),
          Expanded(
            child: Button(
              key: ValueKey('rule-row-${rule.id}'),
              style: AppTheme.historyClipButtonStyle(selected: false),
              alignment: Alignment.centerLeft,
              onPressed: () => unawaited(_edit(rule)),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Wrap(
                    spacing: AppSpacing.xs,
                    runSpacing: AppSpacing.xs,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      Text(
                        rule.name.isEmpty ? l10n.rulesUntitled : rule.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ).medium(),
                      if (rule.errorCode != null)
                        DestructiveBadge(
                          key: ValueKey('rule-error-${rule.id}'),
                          child: Text(l10n.rulesErrorBadge),
                        ),
                      if (rule.unsupported)
                        OutlineBadge(child: Text(l10n.rulesNeedsUpdateBadge)),
                      if (rule.newFromDevice case final device?)
                        SecondaryBadge(
                          child: Text(l10n.rulesNewFromDevice(device)),
                        ),
                    ],
                  ),
                  const Gap(AppSpacing.xxs),
                  Text(
                    summary,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ).small().muted(),
                  if (rule.errorCode case final code?)
                    Text(
                      l10n.rulesErrorHint(l10n.rulesProblem(code)),
                    ).xSmall().muted()
                  else if (rule.unsupported && definition != null)
                    Text(l10n.rulesUpdateRequired).xSmall().muted(),
                  if (devices != null)
                    Text(
                      devices,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ).xSmall().muted(),
                ],
              ),
            ),
          ),
          const Gap(AppSpacing.sm),
          SettingSwitch(
            label: rule.name,
            showState: false,
            toggle: Switch(
              key: ValueKey('rule-enabled-${rule.id}'),
              value: rule.enabled,
              onChanged: _controller.busy
                  ? null
                  : (value) =>
                        _run(() => _controller.setEnabled(rule.id, value)),
            ),
          ),
          Builder(
            builder: (anchor) => _iconButton(
              l10n.rulesMoreActions,
              LucideIcons.ellipsis,
              () => unawaited(_menu(anchor, rule, moves)),
              key: ValueKey('rule-menu-${rule.id}'),
            ),
          ),
        ],
      ),
    );
    return Semantics(
      customSemanticsActions: {
        for (final MapEntry(key: direction, value: label) in moves.entries)
          CustomSemanticsAction(label: label): () => move(direction),
      },
      child: CallbackShortcuts(
        bindings: {
          if (user) ...{
            SingleActivator(
              LogicalKeyboardKey.arrowUp,
              alt: true,
              meta: mod,
              control: !mod,
            ): () =>
                move(RuleMove.up),
            SingleActivator(
              LogicalKeyboardKey.arrowDown,
              alt: true,
              meta: mod,
              control: !mod,
            ): () =>
                move(RuleMove.down),
            const SingleActivator(LogicalKeyboardKey.delete): () =>
                unawaited(_delete(rule)),
            const SingleActivator(LogicalKeyboardKey.backspace): () =>
                unawaited(_delete(rule)),
          },
        },
        child: row,
      ),
    );
  }

  Future<void> _menu(
    BuildContext anchor,
    ContentRule rule,
    Map<RuleMove, String> moves,
  ) {
    final l10n = context.l10n;
    MenuButton item(
      String label,
      IconData icon,
      VoidCallback onPressed, {
      Key? key,
    }) => MenuButton(
      key: key,
      onPressed: (_) => onPressed(),
      leading: AppIconFrame(
        size: AppIconSize.sm,
        child: Icon(icon, size: AppIconSize.sm),
      ),
      child: Text(label),
    );
    return showDropdown<void>(
      context: anchor,
      alignment: AlignmentDirectional.topEnd,
      anchorAlignment: AlignmentDirectional.bottomEnd,
      builder: (popupContext) => AppTheme.popupSurface(
        popupContext,
        child: DropdownMenu(
          children: [
            item(
              l10n.rulesEdit,
              LucideIcons.pencil,
              () => unawaited(_edit(rule)),
            ),
            for (final MapEntry(key: direction, value: label) in moves.entries)
              item(
                label,
                switch (direction) {
                  RuleMove.up => LucideIcons.arrowUp,
                  RuleMove.down => LucideIcons.arrowDown,
                  RuleMove.top => LucideIcons.arrowUpToLine,
                  RuleMove.bottom => LucideIcons.arrowDownToLine,
                },
                () => _run(() => _controller.move(rule.id, direction)),
                key: ValueKey('rule-move-${direction.name}-${rule.id}'),
              ),
            if (!rule.unsupported)
              item(
                l10n.rulesApplyToItems,
                LucideIcons.history,
                () => unawaited(_preview(rule)),
              ),
            if (rule.builtin)
              item(
                l10n.rulesResetBuiltin,
                LucideIcons.rotateCcw,
                () => _run(() => _controller.resetBuiltin(rule.id)),
              )
            else
              item(
                l10n.commonDelete,
                LucideIcons.trash2,
                () => unawaited(_delete(rule)),
                key: ValueKey('rule-delete-${rule.id}'),
              ),
          ],
        ),
      ),
    ).future;
  }

  Widget _iconButton(
    String label,
    IconData icon,
    VoidCallback? onPressed, {
    Key? key,
  }) => Tooltip(
    tooltip: (context) => TooltipContainer(child: Text(label)),
    child: Button.ghost(
      key: key,
      style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
      onPressed: onPressed,
      child: Semantics(
        label: label,
        child: AppIconFrame(child: Icon(icon)),
      ),
    ),
  );

  Widget _deletedGroup(BuildContext context) {
    final l10n = context.l10n;
    return SettingsGroup(
      children: [
        SettingRow(
          title: l10n.rulesDeletedTitle,
          description: l10n.rulesDeletedDescription,
        ),
        for (final rule in _controller.deletedRules)
          Padding(
            padding: AppTheme.settingsRowPadding,
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        rule.name.isEmpty ? l10n.rulesUntitled : rule.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      Text(
                        l10n.rulesDeletedAt(
                          formatSystemRecentTime(context, rule.deletedAt!),
                        ),
                      ).xSmall().muted(),
                    ],
                  ),
                ),
                Button.secondary(
                  key: ValueKey('rule-restore-${rule.id}'),
                  style: AppTheme.controlButtonStyle(
                    const ButtonStyle.secondary(),
                  ),
                  onPressed: _controller.busy
                      ? null
                      : () => _run(() => _controller.restore(rule.id)),
                  child: Text(l10n.rulesRestore),
                ),
              ],
            ),
          ),
      ],
    );
  }

  Widget _skipLogRow(BuildContext context) {
    final l10n = context.l10n;
    return SettingRow(
      key: widget.targets.skipped,
      highlighted: widget.scope.isHighlighted(
        ContentRulesSettingsTargets.skippedId,
      ),
      title: l10n.rulesSkipLogTitle,
      description: l10n.rulesSkipLogDescription,
      trailing: Button.secondary(
        key: const ValueKey('rules-skip-log'),
        style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
        onPressed: () => unawaited(_showSkipLog()),
        child: Text(l10n.rulesSkipLogShow),
      ),
    );
  }
}

/// Captures rules did not save on this device, newest first. Content is
/// never kept, so each row names only the time, app and rule.
class _SkipLogDialog extends StatelessWidget {
  const _SkipLogDialog({required this.events});

  final List<RuleSkipEvent> events;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): () =>
            Navigator.pop(context),
      },
      child: AppOverlays.alertDialog(
        icon: LucideIcons.ban,
        title: Text(l10n.rulesSkipLogTitle),
        content: events.isEmpty
            ? Text(l10n.rulesSkipLogEmpty)
            : Column(
                key: const ValueKey('rules-skip-log-list'),
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  for (final event in events)
                    Padding(
                      padding: const EdgeInsets.only(bottom: AppSpacing.sm),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(event.ruleName).small(),
                          Text(
                            [
                              event.sourceAppName ?? l10n.rulesUnknownApp,
                              formatSystemRecentTime(context, event.at),
                            ].join(' · '),
                          ).xSmall().muted(),
                        ],
                      ),
                    ),
                ],
              ),
        actions: [
          Button.primary(
            alignment: Alignment.center,
            style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
            onPressed: () => Navigator.pop(context),
            child: Text(l10n.commonClose),
          ),
        ],
      ),
    );
  }
}
