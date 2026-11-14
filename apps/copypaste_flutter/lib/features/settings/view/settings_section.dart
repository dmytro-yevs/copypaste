import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';

/// A titled settings section; grouped content shares one card.
class SettingsSection extends StatelessWidget {
  const SettingsSection({
    super.key,
    required this.title,
    required this.description,
    required this.children,
    this.groupContent = true,
    this.showHeading = true,
  });

  final String title;
  final String description;
  final List<Widget> children;
  final bool groupContent;
  final bool showHeading;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (showHeading) ...[
          Text(title, style: Theme.of(context).typography.h3),
          const Gap(AppSpacing.xs),
        ],
        Text(description).muted(),
        const Gap(AppSpacing.lg),
        if (groupContent) SettingsGroup(children: children) else ...children,
      ],
    );
  }
}

/// One card of divided settings rows.
class SettingsGroup extends StatelessWidget {
  const SettingsGroup({super.key, required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Card(
      theme: AppTheme.settingsGroupCardTheme,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final (index, slot) in _slots().indexed) ...[
            if (index > 0) const Divider(),
            slot,
          ],
        ],
      ),
    );
  }

  /// Rows divide the group; notices that follow each other share one slot,
  /// inset like a row's content.
  List<Widget> _slots() {
    final slots = <Widget>[];
    var notices = <Widget>[];
    void closeNotices() {
      if (notices.isEmpty) return;
      slots.add(
        Padding(
          padding: AppTheme.settingsRowPadding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (final (index, notice) in notices.indexed) ...[
                if (index > 0) const Gap(AppTheme.settingsBlockSpacing),
                notice,
              ],
            ],
          ),
        ),
      );
      notices = [];
    }

    for (final child in children) {
      if (child is Alert) {
        notices.add(child);
      } else {
        closeNotices();
        slots.add(child);
      }
    }
    closeNotices();
    return slots;
  }
}

/// Builds a setting's copy with [value] beside it, or under it when text is
/// scaled up.
typedef SettingRowHeader = Widget Function(Widget value);

/// A setting title and description with its control.
///
/// A switch stays beside the copy in a narrow row; other controls stack below
/// the copy when space is narrow, and every control does when text is scaled
/// up.
class SettingRow extends StatelessWidget {
  const SettingRow({
    super.key,
    required this.title,
    required this.description,
    this.trailing,
    this.footer,
    this.scale,
    this.status,
    this.highlighted = false,
  }) : assert(
         scale == null || (trailing == null && footer == null),
         'A scale row names its value beside the copy instead.',
       );

  final String title;
  final String description;
  final Widget? trailing;

  /// The current state of what the row sets, under its description.
  final Widget? status;

  /// A full-width control under the copy.
  final Widget? footer;

  /// A scale under the copy. It lays out the row header it is given, so its
  /// current value stays beside the copy.
  final Widget Function(SettingRowHeader header)? scale;
  final bool highlighted;

  @override
  Widget build(BuildContext context) {
    final copy = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title).medium(),
        const Gap(AppSpacing.xs),
        Text(description).muted().textSmall(),
        if (status case final status?) ...[const Gap(AppSpacing.xs), status],
      ],
    );
    return OutlinedContainer(
      key: ValueKey<String>('settings-row-$title'),
      duration: AppMotion.resolve(context, AppMotion.quick),
      theme: AppTheme.settingsRowTheme(context, highlighted: highlighted),
      clipBehavior: Clip.none,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final narrow =
              constraints.maxWidth < AppLayoutSize.settingsStackedControlWidth;
          final largeText = MediaQuery.textScalerOf(context).scale(1) > 1.3;
          final toggle = trailing;
          final control = toggle is Switch
              ? SettingSwitch(
                  label: title,
                  toggle: toggle,
                  stacked: narrow && !largeText,
                )
              : toggle;
          Widget header(Widget? beside, {required bool stack}) => beside == null
              ? copy
              : stack
              ? Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    copy,
                    const Gap(AppTheme.settingsBlockSpacing),
                    Align(alignment: Alignment.centerRight, child: beside),
                  ],
                )
              : Row(
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: [
                    Expanded(child: copy),
                    const Gap(AppSpacing.lg),
                    beside,
                  ],
                );
          // A switch or a value stays beside its copy in a narrow row, with
          // a switch's state word under it; only enlarged text moves either
          // below the copy.
          if (scale case final scale?) {
            return scale((value) => header(value, stack: largeText));
          }
          final row = header(
            control,
            stack: largeText || (control is! SettingSwitch && narrow),
          );
          if (footer case final footer?) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [row, const Gap(AppTheme.settingsBlockSpacing), footer],
            );
          }
          return row;
        },
      ),
    );
  }
}

/// A row switch named by its row; [AppSwitchFrame] reports and draws its
/// state. A switch without `onChanged` (its change is pending, or another
/// change excludes it) is disabled. Disabling drops keyboard focus, so focus
/// returns to the switch when it can be used again, unless it moved meanwhile.
class SettingSwitch extends StatefulWidget {
  const SettingSwitch({
    super.key,
    required this.label,
    required this.toggle,
    this.showState = true,
    this.stacked = false,
  });

  final String label;
  final Switch toggle;
  final bool showState;
  final bool stacked;

  @override
  State<SettingSwitch> createState() => _SettingSwitchState();
}

class _SettingSwitchState extends State<SettingSwitch> {
  final _focus = FocusNode(
    debugLabel: 'setting-switch',
    canRequestFocus: false,
    skipTraversal: true,
  );
  bool _restoreFocus = false;
  FocusNode? _focusedWhileDisabled;

  bool _enabled(Switch toggle) =>
      toggle.enabled != false && toggle.onChanged != null;

  @override
  void didUpdateWidget(SettingSwitch oldWidget) {
    super.didUpdateWidget(oldWidget);
    final was = _enabled(oldWidget.toggle);
    final now = _enabled(widget.toggle);
    if (was && !now && _focus.hasFocus) {
      _restoreFocus = true;
      _focusedWhileDisabled = null;
      // The focus manager applies the loss in a microtask after this frame.
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => scheduleMicrotask(() {
          final primary = FocusManager.instance.primaryFocus;
          if (_restoreFocus && primary != null && !_focus.hasFocus) {
            _focusedWhileDisabled = primary;
          }
        }),
      );
    } else if (!was && now && _restoreFocus) {
      final moved = _focusedWhileDisabled;
      _restoreFocus = false;
      _focusedWhileDisabled = null;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        final primary = FocusManager.instance.primaryFocus;
        if (!mounted || (moved != null && primary != moved)) return;
        for (final node in _focus.descendants) {
          if (node.canRequestFocus) return node.requestFocus();
        }
      });
    }
  }

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final toggle = widget.toggle;
    return MergeSemantics(
      child: Semantics(
        label: widget.label,
        child: Focus(
          focusNode: _focus,
          includeSemantics: false,
          child: AppSwitchFrame(
            showState: widget.showState,
            stacked: widget.stacked,
            child: Switch(
              key: toggle.key,
              value: toggle.value,
              onChanged: toggle.onChanged,
              enabled: _enabled(toggle),
            ),
          ),
        ),
      ),
    );
  }
}
