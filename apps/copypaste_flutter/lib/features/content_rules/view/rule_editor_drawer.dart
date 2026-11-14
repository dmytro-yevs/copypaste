import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/adaptive_breakpoints.dart';
import '../../snippets/view/snippet_dialogs.dart';
import '../controller/content_rules_controller.dart';
import '../controller/rule_editor_controller.dart';
import '../models/content_rule_models.dart';
import '../presentation/rule_messages.dart';
import 'rule_preview_dialog.dart';
import 'rule_trace_panel.dart';

/// Opens the editor for [rule], or for a new rule prefilled with
/// [definition]. [onDeleted] runs with the id after the rule was deleted.
Future<void> showRuleEditorDrawer(
  BuildContext context, {
  required ContentRulesController controller,
  ContentRule? rule,
  RuleDefinition? definition,
  ValueChanged<String>? onDeleted,
}) => showOverlay<void>(
  context,
  AppOverlays.drawerConfiguration(context, guarded: true),
  builder: (context) => RuleEditorDrawer(
    controller: controller,
    rule: rule,
    definition: definition,
    onDeleted: onDeleted,
  ),
).future;

/// When / Then / Options / Test of one rule, with Save, Cancel and Delete in
/// a fixed footer. The runtime validates the draft as it changes; closing
/// with unsaved changes asks first.
class RuleEditorDrawer extends StatefulWidget {
  const RuleEditorDrawer({
    super.key,
    required this.controller,
    this.rule,
    this.definition,
    this.onDeleted,
  });

  final ContentRulesController controller;
  final ContentRule? rule;
  final RuleDefinition? definition;
  final ValueChanged<String>? onDeleted;

  @override
  State<RuleEditorDrawer> createState() => _RuleEditorDrawerState();
}

class _RuleEditorDrawerState extends State<RuleEditorDrawer> {
  late final RuleEditorController _editor = RuleEditorController(
    repository: widget.controller.repository,
    options: widget.controller.options,
    rule: widget.rule,
    definition: widget.definition,
  );

  /// Stable keys for condition rows, so field state follows a removal.
  late final List<int> _conditionIds = [
    for (var i = 0; i < _editor.definition.conditions.length; i++) i,
  ];
  late int _nextConditionId = _conditionIds.length;
  final _sampleFocus = FocusNode(debugLabel: 'rule-sample');
  bool _closing = false;

  @override
  void dispose() {
    _editor.dispose();
    _sampleFocus.dispose();
    super.dispose();
  }

  Future<void> _close() async {
    if (_closing || _editor.saving) return;
    if (_editor.dirty) {
      final choice = await showUnsavedChangesDialog(context);
      if (!mounted) return;
      switch (choice) {
        case UnsavedChoice.cancel:
          return;
        case UnsavedChoice.save:
          await _save();
          return;
        case UnsavedChoice.discard:
          break;
      }
    }
    _dismiss();
  }

  void _dismiss() {
    if (_closing || !mounted) return;
    _closing = true;
    unawaited(closeDrawer(context));
  }

  Future<void> _save() async {
    if (await _editor.save()) _dismiss();
  }

  Future<void> _delete() async {
    final id = widget.rule?.id;
    if (id == null || !await widget.controller.delete(id) || !mounted) return;
    _dismiss();
    widget.onDeleted?.call(id);
  }

  Future<void> _preview() => showRulePreviewDialog(
    context,
    repository: widget.controller.repository,
    definitionJson: _editor.previewDefinitionJson,
    ruleId: widget.rule?.id,
    fullSet: _editor.fullSet,
  );

  void _addCondition(RuleConditionType type) {
    final before = _editor.definition.conditions.length;
    _editor.addCondition(type);
    if (_editor.definition.conditions.length > before) {
      _conditionIds.add(_nextConditionId++);
    }
  }

  void _removeCondition(int index) {
    _editor.removeCondition(index);
    _conditionIds.removeAt(index);
  }

  static BoxConstraints _constraints(BuildContext context) {
    final size = MediaQuery.sizeOf(context);
    if (size.width < AdaptiveBreakpoints.inspector) {
      return AppOverlays.drawerContentConstraints(context);
    }
    return BoxConstraints.tightFor(
      width: math.min(
        AppOverlaySize.editorPanelMaxWidth,
        math.max(AppOverlaySize.drawerPanelWidth, size.width * 0.6),
      ),
      height: double.infinity,
    );
  }

  @override
  Widget build(BuildContext context) {
    final mod = defaultTargetPlatform == TargetPlatform.macOS;
    SingleActivator chord(LogicalKeyboardKey key) =>
        SingleActivator(key, meta: mod, control: !mod);
    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) unawaited(_close());
      },
      child: AppOverlays.drawerDismissal(
        onDismiss: () => unawaited(_close()),
        child: CallbackShortcuts(
          bindings: {
            chord(LogicalKeyboardKey.keyS): () => unawaited(_save()),
            chord(LogicalKeyboardKey.enter): () => unawaited(_save()),
            chord(LogicalKeyboardKey.keyT): _sampleFocus.requestFocus,
          },
          child: Focus(
            autofocus: true,
            skipTraversal: true,
            child: ListenableBuilder(
              listenable: _editor,
              builder: (context, _) => ConstrainedBox(
                key: const ValueKey('rule-editor-drawer'),
                constraints: _constraints(context),
                child: Scaffold(
                  headers: [_header(context), const Divider()],
                  footers: [const Divider(), _footer(context)],
                  child: SingleChildScrollView(
                    key: const ValueKey('rule-editor-body-scroll'),
                    padding: const EdgeInsets.all(AppSpacing.lg),
                    child: _fields(context),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _header(BuildContext context) {
    final l10n = context.l10n;
    return AppBar(
      title: Text(
        _editor.isNew ? l10n.rulesNewRule : l10n.rulesEditRule,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      leading: const [AppIconFrame(child: Icon(LucideIcons.listFilter))],
      trailing: [
        Tooltip(
          tooltip: (context) => TooltipContainer(child: Text(l10n.commonClose)),
          child: Button.ghost(
            key: const ValueKey('rule-editor-close'),
            style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
            onPressed: () => unawaited(_close()),
            child: Semantics(
              label: l10n.commonClose,
              child: const AppIconFrame(child: Icon(LucideIcons.x)),
            ),
          ),
        ),
      ],
    );
  }

  Widget _footer(BuildContext context) {
    final l10n = context.l10n;
    final rule = widget.rule;
    return Padding(
      padding: const EdgeInsets.all(AppSpacing.lg),
      child: Row(
        children: [
          if (rule != null && !rule.builtin) ...[
            Button.destructive(
              key: const ValueKey('rule-editor-delete'),
              style: AppTheme.actionButtonStyle(
                const ButtonStyle.destructive(),
              ),
              onPressed: _editor.saving ? null : () => unawaited(_delete()),
              child: Text(l10n.commonDelete),
            ),
            const Gap(AppSpacing.md),
          ],
          Expanded(
            child: Button.secondary(
              key: const ValueKey('rule-editor-cancel'),
              style: AppTheme.actionButtonStyle(const ButtonStyle.secondary()),
              alignment: Alignment.center,
              onPressed: _editor.saving ? null : () => unawaited(_close()),
              child: Text(l10n.commonCancel),
            ),
          ),
          const Gap(AppSpacing.md),
          Expanded(
            child: Button.primary(
              key: const ValueKey('rule-editor-save'),
              style: AppTheme.actionButtonStyle(const ButtonStyle.primary()),
              alignment: Alignment.center,
              onPressed: _editor.canSave ? () => unawaited(_save()) : null,
              child: Text(l10n.commonSave),
            ),
          ),
        ],
      ),
    );
  }

  Widget _heading(String title, {Widget? trailing}) => Padding(
    padding: const EdgeInsets.only(bottom: AppSpacing.sm),
    child: Row(
      children: [
        Expanded(
          child: Semantics(header: true, child: Text(title).large().medium()),
        ),
        ?trailing,
      ],
    ),
  );

  Widget _label(String text) => Padding(
    padding: const EdgeInsets.only(bottom: AppSpacing.xs),
    child: Text(text).small(),
  );

  Widget _switch(
    String label,
    bool value,
    ValueChanged<bool> onChanged, {
    Key? key,
  }) => MergeSemantics(
    child: Row(
      children: [
        Expanded(child: Text(label)),
        const Gap(AppSpacing.md),
        AppSwitchFrame(
          child: Switch(
            key: key,
            value: value,
            onChanged: _editor.readOnly || _editor.saving ? null : onChanged,
          ),
        ),
      ],
    ),
  );

  Widget _problemText(String text, Key key) => Padding(
    padding: const EdgeInsets.only(top: AppSpacing.xs),
    child: Semantics(
      liveRegion: true,
      child: Text(
        text,
        key: key,
        style: TextStyle(
          color: AppTheme.statusColor(context, AppStatusTone.error),
        ),
      ).small(),
    ),
  );

  Widget _fields(BuildContext context) {
    final l10n = context.l10n;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (_editor.errorMessage case final error?) ...[
          Alert.destructive(
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
            title: Text(l10n.rulesSaveFailed),
            content: Text(error),
          ),
          const Gap(AppSpacing.md),
        ],
        if (_editor.readOnly) ...[
          Alert(
            key: const ValueKey('rule-editor-read-only'),
            leading: const AppIconFrame(child: Icon(LucideIcons.info)),
            content: Text(l10n.rulesReadOnlyNotice),
          ),
          const Gap(AppSpacing.md),
        ],
        for (final problem in _editor.ruleProblems)
          if (problem.path.isEmpty)
            _problemText(
              l10n.ruleProblem(problem),
              const ValueKey('rule-editor-problem'),
            ),
        _label(l10n.rulesNameLabel),
        TextField(
          key: const ValueKey('rule-editor-name'),
          initialValue: _editor.name,
          enabled: !_editor.readOnly,
          maxLength: RuleLimits.maxName,
          placeholder: Text(l10n.rulesNamePlaceholder),
          decoration: AppOverlays.dialogFieldDecoration(context),
          onChanged: _editor.setName,
        ),
        const Gap(AppSpacing.xl),
        _when(context),
        const Gap(AppSpacing.xl),
        _then(context),
        const Gap(AppSpacing.xl),
        _options(context),
        const Gap(AppSpacing.xl),
        _test(context),
      ],
    );
  }

  Widget _select<T>({
    required Key key,
    required T value,
    required List<T> values,
    required String Function(T) label,
    required ValueChanged<T> onChanged,
  }) => Select<T>(
    key: key,
    value: value,
    enabled: !_editor.readOnly,
    onChanged: (next) {
      if (next != null) onChanged(next);
    },
    itemBuilder: (context, value) =>
        Text(label(value), maxLines: 1, overflow: TextOverflow.ellipsis),
    popup: SelectPopup<T>(
      items: SelectItemList(
        children: [
          for (final value in values)
            SelectItemButton<T>(value: value, child: Text(label(value))),
        ],
      ),
    ).call,
  );

  Widget _multiSelect({
    required Key key,
    required List<String> value,
    required List<String> values,
    required String Function(String) label,
    required ValueChanged<List<String>> onChanged,
  }) => MultiSelect<String>(
    key: key,
    value: value,
    enabled: !_editor.readOnly,
    placeholder: Text(context.l10n.rulesChooseValues),
    onChanged: (next) => onChanged((next ?? const <String>[]).toList()),
    itemBuilder: (context, id) => MultiSelectChip(
      value: id,
      style: AppTheme.selectChipStyle,
      child: Text(label(id), maxLines: 1, overflow: TextOverflow.ellipsis),
    ),
    popup: SelectPopup<String>(
      items: SelectItemList(
        children: [
          for (final id in {...values, ...value})
            SelectItemButton<String>(value: id, child: Text(label(id))),
        ],
      ),
    ).call,
  );

  Widget _addButton({
    required Key key,
    required String label,
    required bool enabled,
    required List<MenuItem> Function() items,
  }) => Builder(
    builder: (anchor) => Button.secondary(
      key: key,
      style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
      leading: const AppIconFrame(child: Icon(LucideIcons.plus)),
      onPressed: enabled
          ? () => showDropdown<void>(
              context: anchor,
              builder: (popupContext) => AppTheme.popupSurface(
                popupContext,
                child: DropdownMenu(children: items()),
              ),
            )
          : null,
      child: Text(label),
    ),
  );

  Widget _when(BuildContext context) {
    final l10n = context.l10n;
    final definition = _editor.definition;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _heading(
          l10n.rulesWhenTitle,
          trailing: _select<RuleMatchMode>(
            key: const ValueKey('rule-editor-match-mode'),
            value: definition.matchMode,
            values: RuleMatchMode.values,
            label: (mode) => l10n.rulesMatchMode(mode.wire),
            onChanged: _editor.setMatchMode,
          ),
        ),
        if (definition.matchesEverything)
          Text(
            l10n.rulesNoConditions,
            key: const ValueKey('rule-editor-matches-everything'),
          ).small().muted(),
        for (final (index, condition) in definition.conditions.indexed) ...[
          _ConditionRow(
            key: ValueKey('rule-condition-${_conditionIds[index]}'),
            index: index,
            condition: condition,
            editor: _editor,
            problems: _editor.conditionProblems(index),
            select: _select,
            multiSelect: _multiSelect,
            problemText: _problemText,
            onRemove: () => setState(() => _removeCondition(index)),
          ),
          const Gap(AppSpacing.sm),
        ],
        for (final problem in _editor.ruleProblems)
          if (problem.path.startsWith('conditions'))
            _problemText(
              l10n.ruleProblem(problem),
              const ValueKey('rule-editor-conditions-problem'),
            ),
        const Gap(AppSpacing.sm),
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: _addButton(
            key: const ValueKey('rule-editor-add-condition'),
            label: l10n.rulesAddCondition,
            enabled: _editor.canAddCondition,
            items: () => [
              for (final type in RuleConditionType.values)
                MenuButton(
                  key: ValueKey('rule-add-condition-${type.wire}'),
                  onPressed: (_) => setState(() => _addCondition(type)),
                  child: Text(l10n.rulesConditionType(type.wire)),
                ),
            ],
          ),
        ),
      ],
    );
  }

  Widget _then(BuildContext context) {
    final l10n = context.l10n;
    final definition = _editor.definition;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _heading(l10n.rulesThenTitle),
        if (definition.actions.isEmpty)
          Text(l10n.rulesNoActions).small().muted(),
        for (final problem in _editor.ruleProblems)
          if (problem.path.startsWith('actions'))
            _problemText(
              l10n.ruleProblem(problem),
              const ValueKey('rule-editor-actions-problem'),
            ),
        for (final (index, action) in definition.actions.indexed) ...[
          _actionRow(context, index, action),
          const Gap(AppSpacing.sm),
        ],
        const Gap(AppSpacing.sm),
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: _addButton(
            key: const ValueKey('rule-editor-add-action'),
            label: l10n.rulesAddAction,
            enabled: !_editor.readOnly,
            items: () => [
              for (final type in RuleActionType.values)
                _actionChoice(context, type),
            ],
          ),
        ),
      ],
    );
  }

  /// An action the rule can take, or the reason it cannot (R-7).
  MenuItem _actionChoice(BuildContext context, RuleActionType type) {
    final l10n = context.l10n;
    final block = _editor.definition.blockFor(type);
    final label = l10n.rulesActionType(type.wire);
    return MenuButton(
      key: ValueKey('rule-add-action-${type.wire}'),
      enabled: block == null,
      onPressed: block == null ? (_) => _editor.addAction(type) : null,
      leading: AppIconFrame(
        size: AppIconSize.sm,
        child: Icon(RuleIcons.action(type), size: AppIconSize.sm),
      ),
      child: block == null
          ? Text(label)
          : Semantics(
              label: '$label. ${l10n.ruleActionBlock(block)}',
              excludeSemantics: true,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(label),
                  Text(
                    l10n.ruleActionBlock(block),
                    key: ValueKey('rule-add-action-reason-${type.wire}'),
                  ).xSmall().muted(),
                ],
              ),
            ),
    );
  }

  Widget _rowCard({required List<Widget> children}) => Card(
    theme: AppTheme.settingsGroupCardTheme,
    padding: const EdgeInsets.all(AppSpacing.md),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: children,
    ),
  );

  Widget _removeButton(String label, Key key, VoidCallback onPressed) =>
      Tooltip(
        tooltip: (context) => TooltipContainer(child: Text(label)),
        child: Button.ghost(
          key: key,
          style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
          onPressed: _editor.readOnly ? null : onPressed,
          child: Semantics(
            label: label,
            child: const AppIconFrame(child: Icon(LucideIcons.x)),
          ),
        ),
      );

  Widget _actionRow(BuildContext context, int index, RuleAction action) {
    final l10n = context.l10n;
    final options = _editor.options;
    final type = action.type;
    void update(RuleAction next) => _editor.updateAction(index, next);
    return _rowCard(
      children: [
        Row(
          children: [
            AppIconFrame(child: Icon(RuleIcons.action(type))),
            const Gap(AppSpacing.sm),
            Expanded(child: Text(l10n.rulesActionType(action.typeName))),
            _removeButton(
              l10n.rulesRemoveAction,
              ValueKey('rule-action-remove-$index'),
              () => _editor.removeAction(index),
            ),
          ],
        ),
        ...switch (type) {
          null => [Text(l10n.rulesUnknownActionNotice).small().muted()],
          RuleActionType.dontSave => [
            Text(l10n.rulesDontSaveWarning).small().muted(),
          ],
          RuleActionType.markSecret || RuleActionType.hideFromAi => [],
          RuleActionType.deleteAfter => [
            const Gap(AppSpacing.sm),
            _select<Duration>(
              key: ValueKey('rule-action-expiry-$index'),
              value: action.expiry,
              values: {...RuleLimits.expiryChoices, action.expiry}.toList()
                ..sort(),
              label: l10n.ruleDuration,
              onChanged: (expiry) => update(action.copyWith(expiry: expiry)),
            ),
            const Gap(AppSpacing.sm),
            _switch(
              l10n.rulesClearClipboardAtExpiry,
              action.clearClipboard,
              (value) => update(action.copyWith(clearClipboard: value)),
            ),
          ],
          RuleActionType.addTags => [
            const Gap(AppSpacing.sm),
            if (options.tags.isEmpty && action.tagIds.isEmpty)
              Text(l10n.rulesNoTags).small().muted()
            else
              _multiSelect(
                key: ValueKey('rule-action-tags-$index'),
                value: action.tagIds,
                values: [for (final tag in options.tags) tag.id],
                label: (id) => options.tagName(id) ?? id,
                onChanged: (ids) => update(action.copyWith(tagIds: ids)),
              ),
          ],
          RuleActionType.transform => [
            const Gap(AppSpacing.sm),
            if (options.transforms.isEmpty && action.transformId.isEmpty)
              Text(l10n.rulesNoTransforms).small().muted()
            else
              _select<String>(
                key: ValueKey('rule-action-transform-$index'),
                value: action.transformId,
                values: {
                  for (final transform in options.transforms) transform.id,
                  if (action.transformId.isNotEmpty) action.transformId,
                }.toList(),
                label: (id) => options.transform(id)?.name ?? id,
                onChanged: (id) => update(action.copyWith(transformId: id)),
              ),
            const Gap(AppSpacing.sm),
            _switch(
              l10n.rulesReplaceClipboard,
              action.replaceClipboard,
              (value) => update(action.copyWith(replaceClipboard: value)),
            ),
          ],
        },
        for (final problem in _editor.actionProblems(index))
          _problemText(
            l10n.ruleProblem(problem),
            ValueKey('rule-action-problem-$index'),
          ),
      ],
    );
  }

  Widget _options(BuildContext context) {
    final l10n = context.l10n;
    final options = _editor.options;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _heading(l10n.rulesOptionsTitle),
        _switch(
          l10n.rulesEnabledSwitch,
          _editor.enabled,
          _editor.setEnabled,
          key: const ValueKey('rule-editor-enabled'),
        ),
        const Gap(AppSpacing.sm),
        _switch(
          l10n.rulesStopProcessing,
          _editor.definition.stopProcessing,
          _editor.setStopProcessing,
          key: const ValueKey('rule-editor-stop'),
        ),
        const Gap(AppSpacing.md),
        _label(l10n.rulesRunsOnLabel),
        _select<bool>(
          key: const ValueKey('rule-editor-scope'),
          value: _editor.allDevices,
          values: const [true, false],
          label: (all) => all ? l10n.rulesAllDevices : l10n.rulesChosenDevices,
          onChanged: _editor.setAllDevices,
        ),
        if (!_editor.allDevices) ...[
          const Gap(AppSpacing.sm),
          _multiSelect(
            key: const ValueKey('rule-editor-devices'),
            value: _editor.deviceIds,
            values: [for (final device in options.devices) device.id],
            label: (id) => options.deviceName(id) ?? id,
            onChanged: _editor.setDevices,
          ),
        ],
      ],
    );
  }

  Widget _test(BuildContext context) {
    final l10n = context.l10n;
    final trace = _editor.trace;
    final rules = widget.controller;
    String? conditionLabel(String ruleId, int index) {
      final definition = ruleId == widget.rule?.id || ruleId == 'draft'
          ? _editor.definition
          : rules.rule(ruleId)?.definition;
      final conditions = definition?.conditions ?? const [];
      if (index >= conditions.length) return null;
      return l10n.ruleCondition(conditions[index], _editor.options);
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _heading(l10n.rulesTestTitle),
        Text(l10n.rulesTestDescription).small().muted(),
        const Gap(AppSpacing.sm),
        TextArea(
          key: const ValueKey('rule-editor-sample'),
          focusNode: _sampleFocus,
          expandableHeight: true,
          initialHeight: AppControlSize.regular * 2,
          placeholder: Text(l10n.rulesSamplePlaceholder),
          decoration: AppOverlays.dialogFieldDecoration(context),
          onChanged: _editor.setSample,
        ),
        const Gap(AppSpacing.sm),
        _switch(
          l10n.rulesTestFullSet,
          _editor.fullSet,
          _editor.setFullSet,
          key: const ValueKey('rule-editor-full-set'),
        ),
        const Gap(AppSpacing.sm),
        Wrap(
          spacing: AppSpacing.sm,
          runSpacing: AppSpacing.sm,
          children: [
            Button.secondary(
              key: const ValueKey('rule-editor-test'),
              style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
              leading: const AppIconFrame(
                child: Icon(LucideIcons.flaskConical),
              ),
              onPressed: _editor.canTest
                  ? () => unawaited(_editor.runTest())
                  : null,
              child: Text(l10n.rulesRunTest),
            ),
            Button.secondary(
              key: const ValueKey('rule-editor-preview'),
              style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
              leading: const AppIconFrame(child: Icon(LucideIcons.history)),
              onPressed: _editor.saveBlock == null && !_editor.validating
                  ? () => unawaited(_preview())
                  : null,
              child: Text(l10n.rulesPreviewHistory),
            ),
          ],
        ),
        if (_editor.testError case final error?)
          _problemText(error, const ValueKey('rule-editor-test-error')),
        if (trace != null) ...[
          const Gap(AppSpacing.md),
          Semantics(
            liveRegion: true,
            child: RuleTracePanel(
              trace: trace,
              options: _editor.options,
              conditionLabel: conditionLabel,
              transformedOutput: _editor.transformedOutput,
            ),
          ),
        ],
      ],
    );
  }
}

typedef _SelectBuilder =
    Widget Function<T>({
      required Key key,
      required T value,
      required List<T> values,
      required String Function(T) label,
      required ValueChanged<T> onChanged,
    });

typedef _MultiSelectBuilder =
    Widget Function({
      required Key key,
      required List<String> value,
      required List<String> values,
      required String Function(String) label,
      required ValueChanged<List<String>> onChanged,
    });

/// One condition: attribute, operator and value, with runtime problems and
/// the notice of platforms that cannot evaluate it.
class _ConditionRow extends StatelessWidget {
  const _ConditionRow({
    super.key,
    required this.index,
    required this.condition,
    required this.editor,
    required this.problems,
    required this.select,
    required this.multiSelect,
    required this.problemText,
    required this.onRemove,
  });

  final int index;
  final RuleCondition condition;
  final RuleEditorController editor;
  final List<RuleProblem> problems;
  final _SelectBuilder select;
  final _MultiSelectBuilder multiSelect;
  final Widget Function(String text, Key key) problemText;
  final VoidCallback onRemove;

  void _update(RuleCondition next) => editor.updateCondition(index, next);

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final type = condition.type;
    final notice = l10n.ruleConditionNotice(type);
    return Card(
      theme: AppTheme.settingsGroupCardTheme,
      padding: const EdgeInsets.all(AppSpacing.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: type == null
                    ? Text(l10n.rulesConditionType(condition.typeName))
                    : select<RuleConditionType>(
                        key: ValueKey('rule-condition-type-$index'),
                        value: type,
                        values: RuleConditionType.values,
                        label: (type) => l10n.rulesConditionType(type.wire),
                        onChanged: (next) =>
                            _update(condition.copyWith(type: next)),
                      ),
              ),
              const Gap(AppSpacing.sm),
              Tooltip(
                tooltip: (context) =>
                    TooltipContainer(child: Text(l10n.rulesRemoveCondition)),
                child: Button.ghost(
                  key: ValueKey('rule-condition-remove-$index'),
                  style: AppTheme.controlButtonStyle(
                    const ButtonStyle.ghostIcon(),
                  ),
                  onPressed: editor.readOnly ? null : onRemove,
                  child: Semantics(
                    label: l10n.rulesRemoveCondition,
                    child: const AppIconFrame(child: Icon(LucideIcons.x)),
                  ),
                ),
              ),
            ],
          ),
          const Gap(AppSpacing.sm),
          ..._value(context, type),
          for (final problem in problems)
            problemText(
              l10n.ruleProblem(problem),
              ValueKey('rule-condition-problem-$index'),
            ),
          if (notice != null) ...[
            const Gap(AppSpacing.sm),
            Alert(
              key: ValueKey('rule-condition-notice-$index'),
              leading: const AppIconFrame(child: Icon(LucideIcons.info)),
              content: Text(notice),
            ),
          ],
        ],
      ),
    );
  }

  List<Widget> _value(BuildContext context, RuleConditionType? type) {
    final l10n = context.l10n;
    final options = editor.options;
    switch (type) {
      case null:
        return [Text(l10n.rulesUnknownConditionNotice).small().muted()];
      case RuleConditionType.contentText:
        final regex = condition.textOp == RuleTextOp.matchesRegex;
        return [
          select<(RuleTextOp, bool)>(
            key: ValueKey('rule-condition-op-$index'),
            value: (condition.textOp, condition.negate),
            values: [
              for (final op in RuleTextOp.values) ...[(op, false), (op, true)],
            ],
            label: (choice) =>
                l10n.ruleTextOperator(choice.$1, negate: choice.$2),
            onChanged: (choice) => _update(
              condition.copyWith(textOp: choice.$1, negate: choice.$2),
            ),
          ),
          const Gap(AppSpacing.sm),
          TextField(
            key: ValueKey('rule-condition-value-$index'),
            initialValue: condition.value,
            enabled: !editor.readOnly,
            autocorrect: false,
            enableSuggestions: false,
            maxLength: RuleLimits.maxPatternChars,
            placeholder: Text(
              regex ? l10n.rulesPatternPlaceholder : l10n.rulesValuePlaceholder,
            ),
            decoration: AppOverlays.dialogFieldDecoration(context),
            onChanged: (value) => _update(condition.copyWith(value: value)),
          ),
          if (regex) ...[
            const Gap(AppSpacing.xs),
            Text(l10n.rulesPatternHint).xSmall().muted(),
          ],
          const Gap(AppSpacing.sm),
          MergeSemantics(
            child: Row(
              children: [
                Expanded(child: Text(l10n.rulesCaseSensitive).small()),
                AppSwitchFrame(
                  child: Switch(
                    value: condition.caseSensitive,
                    onChanged: editor.readOnly
                        ? null
                        : (value) =>
                              _update(condition.copyWith(caseSensitive: value)),
                  ),
                ),
              ],
            ),
          ),
        ];
      case RuleConditionType.size:
        return [
          select<(RuleComparison, bool)>(
            key: ValueKey('rule-condition-op-$index'),
            value: (condition.comparison, condition.negate),
            values: [
              for (final op in RuleComparison.values) ...[
                (op, false),
                (op, true),
              ],
            ],
            label: (choice) =>
                l10n.ruleSizeOperator(choice.$1, negate: choice.$2),
            onChanged: (choice) => _update(
              condition.copyWith(comparison: choice.$1, negate: choice.$2),
            ),
          ),
          const Gap(AppSpacing.sm),
          Row(
            children: [
              Expanded(
                child: TextField(
                  key: ValueKey('rule-condition-size-$index'),
                  initialValue: '${condition.size}',
                  enabled: !editor.readOnly,
                  keyboardType: TextInputType.number,
                  inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                  decoration: AppOverlays.dialogFieldDecoration(context),
                  onChanged: (value) => _update(
                    condition.copyWith(size: int.tryParse(value) ?? 0),
                  ),
                ),
              ),
              const Gap(AppSpacing.sm),
              select<RuleSizeUnit>(
                key: ValueKey('rule-condition-unit-$index'),
                value: condition.sizeUnit,
                values: RuleSizeUnit.values,
                label: (unit) => l10n.rulesSizeUnit(unit.wire),
                onChanged: (unit) =>
                    _update(condition.copyWith(sizeUnit: unit)),
              ),
            ],
          ),
        ];
      case RuleConditionType.sourceApp:
        return [
          _listOperator(context),
          const Gap(AppSpacing.sm),
          multiSelect(
            key: ValueKey('rule-condition-values-$index'),
            value: condition.values,
            values: [for (final app in options.sourceApps) app.id],
            label: (id) => options.sourceAppName(id) ?? id,
            onChanged: (values) => _update(condition.copyWith(values: values)),
          ),
        ];
      case final RuleConditionType listType:
        return [
          _listOperator(context),
          const Gap(AppSpacing.sm),
          multiSelect(
            key: ValueKey('rule-condition-values-$index'),
            value: condition.values,
            values: ruleChoicesFor(listType),
            label: (wire) => l10n.ruleConditionValue(listType, wire),
            onChanged: (values) => _update(condition.copyWith(values: values)),
          ),
        ];
    }
  }

  Widget _listOperator(BuildContext context) => select<bool>(
    key: ValueKey('rule-condition-op-$index'),
    value: condition.negate,
    values: const [false, true],
    label: (negate) => context.l10n.rulesListOperator(negate.toString()),
    onChanged: (negate) => _update(condition.copyWith(negate: negate)),
  );
}
