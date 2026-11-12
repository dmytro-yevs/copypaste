import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../settings/controller/settings_navigation_state.dart';
import '../../settings/view/settings_search_target.dart';
import '../../settings/view/settings_section.dart';
import '../../settings/view/shortcut_recorder.dart';
import '../controller/paste_queue_controller.dart';
import '../controller/paste_queue_shortcuts_controller.dart';
import '../models/paste_queue_models.dart';
import '../presentation/paste_queue_messages.dart';

/// Paste Queue settings: in Quick Paste on desktop, where [shortcuts] adds
/// the global shortcuts and delivery style, and in Quick access on Android.
class PasteQueueSettingsContribution {
  PasteQueueSettingsContribution(
    this.controller, {
    this.shortcuts,
    TargetPlatform? platform,
  }) : section = (platform ?? defaultTargetPlatform) == TargetPlatform.android
           ? SettingsSectionId.quickAccess
           : SettingsSectionId.quickPaste;

  static const enabledTargetId = 'result-paste-queue-enabled';
  static const shortcutsTargetId = 'result-paste-queue-shortcuts';
  static const separatorTargetId = 'result-paste-queue-separator';
  static const idleTargetId = 'result-paste-queue-idle-stop';

  final PasteQueueController controller;
  final PasteQueueShortcutsController? shortcuts;
  final SettingsSectionId section;
  final _keys = PasteQueueSettingsKeys();

  SettingsFeatureSection get feature => SettingsFeatureSection(
    section: section,
    builder: (context, scope) => PasteQueueSettingsView(
      controller: controller,
      shortcuts: shortcuts,
      scope: scope,
      keys: _keys,
    ),
    searchTargets: (context) {
      final l10n = context.l10n;
      SettingsSearchTarget target(
        String id,
        String label,
        String description,
        GlobalKey key,
      ) => SettingsSearchTarget(
        id: id,
        section: section,
        label: label,
        description: description,
        keywords: l10n.pasteQueueSettingsKeywords,
        targetKey: key,
      );
      return [
        target(
          enabledTargetId,
          l10n.pasteQueueEnableTitle,
          l10n.pasteQueueEnableDescription,
          _keys.enabled,
        ),
        if (shortcuts != null)
          target(
            shortcutsTargetId,
            l10n.pasteQueueShortcutToggleCollecting,
            l10n.pasteQueueShortcutDescription,
            _keys.shortcuts,
          ),
        target(
          separatorTargetId,
          l10n.pasteQueueSeparatorTitle,
          l10n.pasteQueueSeparatorDescription,
          _keys.separator,
        ),
        target(
          idleTargetId,
          l10n.pasteQueueIdleStopTitle,
          l10n.pasteQueueIdleStopDescription,
          _keys.idle,
        ),
      ];
    },
  );
}

/// Rows that settings search scrolls to.
class PasteQueueSettingsKeys {
  final enabled = GlobalKey(debugLabel: 'paste-queue-enabled');
  final shortcuts = GlobalKey(debugLabel: 'paste-queue-shortcuts');
  final separator = GlobalKey(debugLabel: 'paste-queue-separator');
  final idle = GlobalKey(debugLabel: 'paste-queue-idle-stop');
}

class PasteQueueSettingsView extends StatelessWidget {
  const PasteQueueSettingsView({
    super.key,
    required this.controller,
    required this.scope,
    required this.keys,
    this.shortcuts,
  });

  final PasteQueueController controller;
  final PasteQueueShortcutsController? shortcuts;
  final SettingsSectionScope scope;
  final PasteQueueSettingsKeys keys;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: Listenable.merge([controller, ?shortcuts]),
    builder: (context, _) {
      final l10n = context.l10n;
      final settings = controller.settings;
      final ready = controller.status == PasteQueueLoadStatus.ready;
      final options = ready && settings.enabled;
      void update(PasteQueueSettings next) =>
          unawaited(controller.updateSettings(next));
      final shortcuts = this.shortcuts;
      return SettingsSection(
        title: l10n.pasteQueueSettingsTitle,
        description: l10n.pasteQueueSettingsSummary,
        children: [
          SettingRow(
            key: keys.enabled,
            highlighted: scope.isHighlighted(
              PasteQueueSettingsContribution.enabledTargetId,
            ),
            title: l10n.pasteQueueEnableTitle,
            description: l10n.pasteQueueEnableDescription,
            trailing: Switch(
              key: const ValueKey('paste-queue-enabled'),
              value: settings.enabled,
              onChanged: ready
                  ? (value) => update(settings.copyWith(enabled: value))
                  : null,
            ),
          ),
          if (controller.errorCode != null)
            Alert.destructive(
              key: const ValueKey('paste-queue-settings-error'),
              leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
              title: Text(l10n.pasteQueueFailed),
            ),
          if (shortcuts != null) ...[
            if (shortcuts.needsShortcuts)
              Alert(
                key: const ValueKey('paste-queue-settings-shortcut-prompt'),
                leading: const AppIconFrame(child: Icon(LucideIcons.keyboard)),
                title: Text(l10n.pasteQueueShortcutPromptTitle),
                content: Text(l10n.pasteQueueShortcutPromptMessage),
              ),
            for (final (index, action)
                in PasteQueueShortcutAction.values.indexed)
              _shortcutRow(
                context,
                shortcuts,
                action,
                key: index == 0 ? keys.shortcuts : null,
                enabled: options,
              ),
            SettingRow(
              title: l10n.pasteQueuePasteNextStyleTitle,
              description: l10n.pasteQueuePasteNextStyleDescription,
              trailing: _select<PasteNextStyle>(
                key: 'paste-queue-style',
                value: shortcuts.pasteNextStyle,
                values: PasteNextStyle.values,
                label: l10n.pasteQueueStyleName,
                onChanged: options
                    ? (style) => unawaited(shortcuts.setPasteNextStyle(style))
                    : null,
              ),
            ),
            if (controller.banner case final banner?)
              Alert(
                key: const ValueKey('paste-queue-settings-banner'),
                leading: AppIconFrame(
                  child: Icon(
                    banner == PasteQueueBanner.accessibilityMissing
                        ? LucideIcons.accessibility
                        : LucideIcons.clipboard,
                  ),
                ),
                title: Text(l10n.pasteQueueCopyOnlyTitle),
                content: Text(
                  banner == PasteQueueBanner.accessibilityMissing
                      ? l10n.pasteQueueAccessibilityMessage
                      : l10n.pasteQueuePasteUnsupported,
                ),
              ),
          ],
          SettingRow(
            title: l10n.pasteQueueInsertTitle,
            description: l10n.pasteQueueInsertDescription,
            trailing: _select<bool>(
              key: 'paste-queue-insert',
              value: settings.insertAtStart,
              values: const [false, true],
              label: (atStart) => atStart
                  ? l10n.pasteQueueInsertStart
                  : l10n.pasteQueueInsertEnd,
              onChanged: options
                  ? (atStart) =>
                        update(settings.copyWith(insertAtStart: atStart))
                  : null,
            ),
          ),
          SettingRow(
            title: l10n.pasteQueueEndBehaviorTitle,
            description: l10n.pasteQueueEndBehaviorDescription,
            trailing: _select<PasteQueueEndBehavior>(
              key: 'paste-queue-end',
              value: settings.endBehavior,
              values: PasteQueueEndBehavior.values,
              label: l10n.pasteQueueEndBehaviorName,
              onChanged: options
                  ? (behavior) =>
                        update(settings.copyWith(endBehavior: behavior))
                  : null,
            ),
          ),
          SettingRow(
            key: keys.separator,
            highlighted: scope.isHighlighted(
              PasteQueueSettingsContribution.separatorTargetId,
            ),
            title: l10n.pasteQueueSeparatorTitle,
            description: l10n.pasteQueueSeparatorDescription,
            trailing: _select<PasteQueueSeparator>(
              key: 'paste-queue-settings-separator',
              value: settings.separator,
              values: PasteQueueSeparator.values,
              label: l10n.pasteQueueSeparatorName,
              onChanged: options
                  ? (separator) =>
                        update(settings.copyWith(separator: separator))
                  : null,
            ),
            footer: settings.separator == PasteQueueSeparator.custom
                ? _CustomSeparatorField(
                    value: settings.customSeparator,
                    enabled: options,
                    onSubmitted: (value) =>
                        update(settings.copyWith(customSeparator: value)),
                  )
                : null,
          ),
          SettingRow(
            title: l10n.pasteQueuePlainTextTitle,
            description: l10n.pasteQueuePlainTextDescription,
            trailing: Switch(
              value: settings.plainText,
              onChanged: options
                  ? (value) => update(settings.copyWith(plainText: value))
                  : null,
            ),
          ),
          SettingRow(
            key: keys.idle,
            highlighted: scope.isHighlighted(
              PasteQueueSettingsContribution.idleTargetId,
            ),
            title: l10n.pasteQueueIdleStopTitle,
            description: l10n.pasteQueueIdleStopDescription,
            trailing: _select<int>(
              key: 'paste-queue-idle-stop',
              value: settings.idleStopMinutes,
              values: PasteQueueSettings.idleStopOptions,
              label: l10n.pasteQueueIdleStopName,
              onChanged: options
                  ? (minutes) =>
                        update(settings.copyWith(idleStopMinutes: minutes))
                  : null,
            ),
          ),
          SettingRow(
            title: l10n.pasteQueueMaxEntriesTitle,
            description: l10n.pasteQueueMaxEntriesDescription,
            trailing: _select<int>(
              key: 'paste-queue-max-entries',
              value: settings.maxEntries,
              values: {
                ...PasteQueueSettings.maxEntriesOptions,
                settings.maxEntries,
              }.toList()..sort(),
              label: (count) => '$count',
              onChanged: options
                  ? (count) => update(settings.copyWith(maxEntries: count))
                  : null,
            ),
          ),
          SettingRow(
            title: l10n.pasteQueueRestoreTitle,
            description: l10n.pasteQueueRestoreDescription,
            trailing: Switch(
              value: settings.restoreWhenFinished,
              onChanged: options
                  ? (value) =>
                        update(settings.copyWith(restoreWhenFinished: value))
                  : null,
            ),
          ),
        ],
      );
    },
  );

  Widget _shortcutRow(
    BuildContext context,
    PasteQueueShortcutsController shortcuts,
    PasteQueueShortcutAction action, {
    required bool enabled,
    Key? key,
  }) {
    final l10n = context.l10n;
    final problem = shortcuts.problemOf(action);
    return SettingRow(
      key: key,
      highlighted:
          key != null &&
          scope.isHighlighted(PasteQueueSettingsContribution.shortcutsTargetId),
      title: l10n.pasteQueueShortcutAction(action),
      description: l10n.pasteQueueShortcutDescription,
      status: problem == null
          ? null
          : Text(
              l10n.pasteQueueShortcutProblem(problem),
              key: ValueKey('paste-queue-shortcut-problem-${action.name}'),
              style: TextStyle(
                color: AppTheme.statusColor(context, AppStatusTone.warning),
              ),
            ).small(),
      trailing: ShortcutRecorder(
        key: ValueKey('paste-queue-shortcut-${action.name}'),
        shortcut: shortcuts.shortcutFor(action),
        enabled: enabled && !shortcuts.busy,
        onBeginRecording: () => shortcuts.beginRecording(action),
        onCancelRecording: () => shortcuts.cancelRecording(action),
        onRecorded: (shortcut) =>
            unawaited(shortcuts.setShortcut(action, shortcut)),
        onReset: () => unawaited(shortcuts.resetShortcut(action)),
        accepts: shortcuts.accepts,
        onRejected: (_) => shortcuts.reportRejected(action),
      ),
    );
  }

  static Widget _select<T>({
    required String key,
    required T value,
    required List<T> values,
    required String Function(T value) label,
    required ValueChanged<T>? onChanged,
  }) => Select<T>(
    key: ValueKey(key),
    value: value,
    enabled: onChanged != null,
    onChanged: (selected) {
      if (selected != null) onChanged?.call(selected);
    },
    itemBuilder: (context, value) => Text(label(value)),
    popup: SelectPopup<T>(
      items: SelectItemList(
        children: [
          for (final option in values)
            SelectItemButton<T>(value: option, child: Text(label(option))),
        ],
      ),
    ).call,
  );
}

/// The custom separator, saved when it is submitted or loses focus.
class _CustomSeparatorField extends StatefulWidget {
  const _CustomSeparatorField({
    required this.value,
    required this.enabled,
    required this.onSubmitted,
  });

  final String value;
  final bool enabled;
  final ValueChanged<String> onSubmitted;

  @override
  State<_CustomSeparatorField> createState() => _CustomSeparatorFieldState();
}

class _CustomSeparatorFieldState extends State<_CustomSeparatorField> {
  late final _text = TextEditingController(text: widget.value);
  final _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() {
      if (!_focus.hasFocus) _submit();
    });
  }

  @override
  void didUpdateWidget(_CustomSeparatorField oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.value != oldWidget.value && !_focus.hasFocus) {
      _text.text = widget.value;
    }
  }

  @override
  void dispose() {
    _text.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _submit() {
    if (_text.text != widget.value) widget.onSubmitted(_text.text);
  }

  @override
  Widget build(BuildContext context) => TextField(
    key: const ValueKey('paste-queue-custom-separator'),
    controller: _text,
    focusNode: _focus,
    enabled: widget.enabled,
    maxLength: PasteQueueSettings.customSeparatorMaxLength,
    placeholder: Text(context.l10n.pasteQueueCustomSeparatorPlaceholder),
    onSubmitted: (_) => _submit(),
  );
}
