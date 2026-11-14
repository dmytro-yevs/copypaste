import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../history/models/history_models.dart';
import '../../history/presentation/source_app_label.dart';
import '../../settings/view/settings_search_target.dart';
import '../../settings/view/settings_section.dart';
import '../controller/ai_tools_controller.dart';
import '../models/ai_tools_models.dart';
import '../presentation/ai_tools_labels.dart';
import 'ai_scope_fields.dart';
import 'ai_tools_settings_targets.dart';

/// What AI tools can see: the synced exposure policy, editable on every
/// platform. Sensitive items are never shared, whatever it allows.
class AiPolicyEditor extends StatefulWidget {
  const AiPolicyEditor({
    super.key,
    required this.controller,
    required this.targets,
    required this.scope,
  });

  final AiToolsController controller;
  final AiToolsSettingsTargets targets;
  final SettingsSectionScope scope;

  @override
  State<AiPolicyEditor> createState() => _AiPolicyEditorState();
}

class _AiPolicyEditorState extends State<AiPolicyEditor> {
  final _icons = <String, Future<HistorySourceAppIcon?>>{};

  AiToolsController get _controller => widget.controller;

  bool _highlighted(String id) => widget.scope.isHighlighted(id);

  void _run(Future<bool> Function() change) => unawaited(change());

  Future<HistorySourceAppIcon?> _icon(String? iconId) {
    if (iconId == null) return Future.value();
    return _icons.putIfAbsent(
      iconId,
      () => _controller.sourceAppIcon(iconId).catchError((Object _) => null),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final policy = _controller.policy;
    final busy = _controller.busy;
    final targets = widget.targets;
    return SettingsGroup(
      children: [
        Alert(
          leading: const AppIconFrame(child: Icon(LucideIcons.eyeOff)),
          title: Text(l10n.aiToolsPolicyTitle),
          content: Text(l10n.aiToolsPolicyNote),
        ),
        SettingRow(
          key: targets.devices,
          highlighted: _highlighted(AiToolsSettingsTargets.devicesId),
          title: l10n.aiToolsOtherDevicesTitle,
          description: l10n.aiToolsOtherDevicesDescription,
          trailing: Switch(
            key: const ValueKey('ai-policy-other-devices'),
            value: policy.includeOtherDevices,
            onChanged: busy
                ? null
                : (value) =>
                      _run(() => _controller.setIncludeOtherDevices(value)),
          ),
          footer: _controller.options.devices.isEmpty
              ? null
              : Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (final device in _controller.options.devices)
                      _switchLine(
                        label: device.label,
                        value: !policy.excludedDeviceIds.contains(device.id),
                        onChanged: busy
                            ? null
                            : (value) => _run(
                                () => _controller.setDeviceIncluded(
                                  device.id,
                                  value,
                                ),
                              ),
                      ),
                  ],
                ),
        ),
        SettingRow(
          key: targets.kinds,
          highlighted: _highlighted(AiToolsSettingsTargets.kindsId),
          title: l10n.aiToolsKindsTitle,
          description: l10n.aiToolsKindsDescription,
          footer: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (final kind in AiToolContentKind.values)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: AppSpacing.xxs),
                  child: Checkbox(
                    key: ValueKey('ai-policy-kind-${kind.name}'),
                    state: policy.contentKinds.contains(kind)
                        ? CheckboxState.checked
                        : CheckboxState.unchecked,
                    onChanged: busy
                        ? null
                        : (state) => _run(
                            () => _controller.setContentKind(
                              kind,
                              state == CheckboxState.checked,
                            ),
                          ),
                    trailing: Flexible(
                      child: Text(l10n.aiToolsContentKind(kind)),
                    ),
                  ),
                ),
            ],
          ),
        ),
        _excludedAppsRow(context),
        SettingRow(
          key: targets.lookback,
          highlighted: _highlighted(AiToolsSettingsTargets.lookbackId),
          title: l10n.aiToolsDefaultLookbackTitle,
          description: l10n.aiToolsDefaultLookbackDescription,
          trailing: AiLookbackSelect(
            key: const ValueKey('ai-policy-lookback'),
            value: policy.defaultLookback,
            enabled: !busy,
            onChanged: (lookback) =>
                _run(() => _controller.setDefaultLookback(lookback)),
          ),
        ),
      ],
    );
  }

  Widget _switchLine({
    required String label,
    required bool value,
    required ValueChanged<bool>? onChanged,
  }) => Padding(
    padding: const EdgeInsets.symmetric(vertical: AppSpacing.xs),
    child: MergeSemantics(
      child: Row(
        children: [
          Expanded(child: Text(label).small()),
          AppSwitchFrame(
            child: Switch(value: value, onChanged: onChanged),
          ),
        ],
      ),
    ),
  );

  Widget _excludedAppsRow(BuildContext context) {
    final l10n = context.l10n;
    final busy = _controller.busy;
    final excluded = _controller.policy.excludedSourceApps;
    final known = {
      for (final app in _controller.options.sourceApps) app.id: app,
    };
    final available = [
      for (final app in _controller.options.sourceApps)
        if (!excluded.contains(app.id)) app,
    ];
    return SettingRow(
      key: widget.targets.apps,
      highlighted: _highlighted(AiToolsSettingsTargets.appsId),
      title: l10n.aiToolsExcludedAppsTitle,
      description: l10n.aiToolsExcludedAppsDescription,
      footer: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final id in excluded)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: AppSpacing.xxs),
              child: Row(
                children: [
                  Expanded(
                    child: SourceAppLabel(
                      name: known[id]?.label ?? id,
                      icon: _icon(known[id]?.iconId),
                    ),
                  ),
                  Tooltip(
                    tooltip: (_) =>
                        TooltipContainer(child: Text(l10n.aiToolsRemove)),
                    child: Button.ghost(
                      style: AppTheme.controlButtonStyle(
                        const ButtonStyle.ghostIcon(),
                      ),
                      onPressed: busy
                          ? null
                          : () => _run(
                              () => _controller.setAppExcluded(id, false),
                            ),
                      child: Semantics(
                        label: l10n.aiToolsRemove,
                        child: const AppIconFrame(child: Icon(LucideIcons.x)),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          if (excluded.isNotEmpty) const Gap(AppSpacing.sm),
          Select<String>(
            key: ValueKey('ai-policy-add-app-${excluded.length}'),
            enabled: !busy && available.isNotEmpty,
            placeholder: Text(l10n.aiToolsAddApp),
            onChanged: (id) {
              if (id != null) _run(() => _controller.setAppExcluded(id, true));
            },
            itemBuilder: (context, id) => Text(known[id]?.label ?? id),
            popup: SelectPopup<String>(
              items: SelectItemList(
                children: [
                  for (final app in available)
                    SelectItemButton<String>(
                      value: app.id,
                      child: SourceAppLabel(
                        name: app.label,
                        icon: _icon(app.iconId),
                      ),
                    ),
                ],
              ),
            ).call,
          ),
        ],
      ),
    );
  }
}
