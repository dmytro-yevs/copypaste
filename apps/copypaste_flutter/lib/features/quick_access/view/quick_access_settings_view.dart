import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../l10n/l10n.dart';
import '../../../platform/android/android_quick_access_gateway.dart';
import '../../../shared/state_view.dart';
import '../../settings/view/settings_search_target.dart';
import '../../settings/view/settings_section.dart';
import '../controller/quick_access_controller.dart';
import '../models/quick_access_capture_tier.dart';
import 'quick_access_settings_section.dart';

/// Android quick access settings: the clips keyboard, its capture tier and
/// options, Quick Settings tiles and the home-screen widget.
class QuickAccessSettingsView extends StatefulWidget {
  const QuickAccessSettingsView({
    super.key,
    required this.controller,
    required this.targets,
    this.scope = const SettingsSectionScope(highlightedTargetId: null),
    this.capturePaused = false,
  });

  final QuickAccessController controller;
  final QuickAccessSettingsTargets targets;
  final SettingsSectionScope scope;
  final bool capturePaused;

  @override
  State<QuickAccessSettingsView> createState() =>
      _QuickAccessSettingsViewState();
}

class _QuickAccessSettingsViewState extends State<QuickAccessSettingsView>
    with WidgetsBindingObserver {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    unawaited(widget.controller.refresh());
  }

  // System settings, the keyboard picker and placement prompts change the
  // state outside the app; it is read again when the app regains focus.
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(widget.controller.refresh());
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: widget.controller,
    builder: (context, _) {
      final controller = widget.controller;
      final l10n = context.l10n;
      final state = controller.state;
      if (state == null) {
        return controller.failure == QuickAccessFailure.load
            ? StateView.error(
                title: l10n.quickAccessUnavailableTitle,
                message: l10n.quickAccessUnavailableMessage,
                actionLabel: l10n.commonRetry,
                onAction: () => unawaited(controller.refresh()),
              )
            : const StateView.loading();
      }
      return SettingsGroup(
        children: [
          if (controller.failure == QuickAccessFailure.action)
            Alert(
              leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
              title: Text(l10n.quickAccessActionFailedTitle),
              content: Text(l10n.quickAccessActionFailedMessage),
            ),
          ..._keyboardRows(context, state),
          ..._tileRows(context, state),
          _widgetRow(context, state),
        ],
      );
    },
  );

  bool get _idle => !widget.controller.busy;

  bool _highlighted(String id) => widget.scope.isHighlighted(id);

  Widget _status(String text, {Key? key}) => Text(text, key: key).textSmall();

  Widget _action(String label, {Key? key, VoidCallback? onPressed}) =>
      Button.secondary(
        key: key,
        style: AppTheme.actionButtonStyle(const ButtonStyle.secondary()),
        onPressed: _idle ? onPressed : null,
        child: Text(label),
      );

  List<Widget> _keyboardRows(
    BuildContext context,
    AndroidQuickAccessState state,
  ) {
    final l10n = context.l10n;
    final controller = widget.controller;
    final targets = widget.targets;
    return [
      SettingRow(
        key: targets.keyboard,
        highlighted: _highlighted(QuickAccessSettingsTargets.keyboardId),
        title: l10n.quickAccessKeyboardTitle,
        description: l10n.quickAccessKeyboardDescription,
        status: _status(
          state.keyboardEnabled
              ? l10n.quickAccessKeyboardOn
              : l10n.quickAccessKeyboardOff,
          key: const ValueKey<String>('quick-access-keyboard-status'),
        ),
        trailing: _action(
          l10n.quickAccessOpenKeyboardSettings,
          key: const ValueKey<String>('quick-access-keyboard-settings'),
          onPressed: () => unawaited(controller.openKeyboardSettings()),
        ),
      ),
      SettingRow(
        title: l10n.quickAccessSwitchKeyboardTitle,
        description: l10n.quickAccessSwitchKeyboardDescription,
        status: state.keyboardSelected
            ? _status(l10n.quickAccessKeyboardCurrent)
            : null,
        trailing: _action(
          l10n.quickAccessSwitchKeyboardAction,
          key: const ValueKey<String>('quick-access-keyboard-picker'),
          onPressed: state.keyboardEnabled
              ? () => unawaited(controller.showKeyboardPicker())
              : null,
        ),
      ),
      SettingRow(
        key: targets.captureTier,
        highlighted: _highlighted(QuickAccessSettingsTargets.captureTierId),
        title: l10n.quickAccessCaptureTierTitle,
        description: l10n.quickAccessCaptureTierDescription,
        status: _status(
          widget.capturePaused
              ? l10n.quickAccessTierPaused
              : switch (QuickAccessCaptureTier.of(state)) {
                  QuickAccessCaptureTier.full => l10n.quickAccessTierFull,
                  QuickAccessCaptureTier.panelCollecting =>
                    l10n.quickAccessTierPanelCollecting,
                  QuickAccessCaptureTier.panelCatchUp =>
                    l10n.quickAccessTierPanelCatchUp,
                  QuickAccessCaptureTier.manual => l10n.quickAccessTierManual,
                },
          key: const ValueKey<String>('quick-access-capture-tier'),
        ),
      ),
      SettingRow(
        key: targets.autoReturn,
        highlighted: _highlighted(QuickAccessSettingsTargets.autoReturnId),
        title: l10n.quickAccessAutoReturnTitle,
        description: l10n.quickAccessAutoReturnDescription,
        trailing: _optionSwitch(state, AndroidPanelOption.autoReturn),
      ),
      SettingRow(
        title: l10n.quickAccessFocusSearchTitle,
        description: l10n.quickAccessFocusSearchDescription,
        trailing: _optionSwitch(state, AndroidPanelOption.focusSearchOnOpen),
      ),
    ];
  }

  Switch _optionSwitch(
    AndroidQuickAccessState state,
    AndroidPanelOption option,
  ) => Switch(
    key: ValueKey<String>('quick-access-option-${option.wire}'),
    value: state.option(option),
    onChanged: _idle
        ? (enabled) =>
              unawaited(widget.controller.setPanelOption(option, enabled))
        : null,
  );

  List<Widget> _tileRows(BuildContext context, AndroidQuickAccessState state) {
    final l10n = context.l10n;
    final highlighted = _highlighted(QuickAccessSettingsTargets.tilesId);
    if (!state.tileRequestSupported) {
      return [
        SettingRow(
          key: widget.targets.tiles,
          highlighted: highlighted,
          title: l10n.quickAccessTilesTitle,
          description: l10n.quickAccessTileInstructions,
        ),
      ];
    }
    return [
      for (final (index, tile) in AndroidQuickAccessTile.values.indexed)
        _tileRow(
          context,
          tile,
          key: index == 0 ? widget.targets.tiles : null,
          highlighted: highlighted,
        ),
    ];
  }

  Widget _tileRow(
    BuildContext context,
    AndroidQuickAccessTile tile, {
    Key? key,
    required bool highlighted,
  }) {
    final l10n = context.l10n;
    final result = widget.controller.tileResult(tile);
    final placed =
        result == AndroidTileRequestResult.added ||
        result == AndroidTileRequestResult.alreadyAdded;
    // After a denial or a failed prompt the system is not asked again; the
    // row explains how to place the tile by hand.
    final refused =
        result == AndroidTileRequestResult.notAdded ||
        result == AndroidTileRequestResult.unavailable;
    final (title, description) = switch (tile) {
      AndroidQuickAccessTile.saveClipboard => (
        l10n.quickAccessSaveTileTitle,
        l10n.quickAccessSaveTileDescription,
      ),
      AndroidQuickAccessTile.captureToggle => (
        l10n.quickAccessCaptureTileTitle,
        l10n.quickAccessCaptureTileDescription,
      ),
    };
    return SettingRow(
      key: key,
      highlighted: highlighted,
      title: title,
      description: refused ? l10n.quickAccessTileInstructions : description,
      status: placed ? _status(l10n.quickAccessTileAdded) : null,
      trailing: placed || refused
          ? null
          : _action(
              l10n.quickAccessAddTile,
              key: ValueKey<String>('quick-access-add-tile-${tile.wire}'),
              onPressed: () =>
                  unawaited(widget.controller.requestAddTile(tile)),
            ),
    );
  }

  Widget _widgetRow(BuildContext context, AndroidQuickAccessState state) {
    final l10n = context.l10n;
    return SettingRow(
      key: widget.targets.widget,
      highlighted: _highlighted(QuickAccessSettingsTargets.widgetId),
      title: l10n.quickAccessWidgetTitle,
      description: state.widgetPinSupported
          ? l10n.quickAccessWidgetDescription
          : l10n.quickAccessWidgetInstructions,
      status: _status(
        l10n.quickAccessWidgetCount(state.widgetCount),
        key: const ValueKey<String>('quick-access-widget-count'),
      ),
      trailing: state.widgetPinSupported
          ? _action(
              l10n.quickAccessAddWidget,
              key: const ValueKey<String>('quick-access-add-widget'),
              onPressed: () => unawaited(widget.controller.requestPinWidget()),
            )
          : null,
    );
  }
}
