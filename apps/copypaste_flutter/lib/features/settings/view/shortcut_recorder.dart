import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../platform/desktop/global_shortcut.dart';

/// Records one global shortcut from the next valid key press.
///
/// The owner releases the active registration in [onBeginRecording], restores
/// it in [onCancelRecording], and persists the result of [onRecorded].
class ShortcutRecorder extends StatefulWidget {
  const ShortcutRecorder({
    super.key,
    required this.shortcut,
    required this.onBeginRecording,
    required this.onCancelRecording,
    required this.onRecorded,
    this.onReset,
    this.accepts,
    this.onRejected,
    this.enabled = true,
  });

  /// The bound shortcut, or `null` while unbound.
  final DesktopShortcut? shortcut;
  final Future<bool> Function() onBeginRecording;
  final Future<void> Function() onCancelRecording;
  final ValueChanged<DesktopShortcut> onRecorded;
  final VoidCallback? onReset;

  /// Extra owner policy; a refused combination keeps recording.
  final bool Function(DesktopShortcut shortcut)? accepts;
  final ValueChanged<DesktopShortcut>? onRejected;
  final bool enabled;

  @override
  State<ShortcutRecorder> createState() => _ShortcutRecorderState();
}

class _ShortcutRecorderState extends State<ShortcutRecorder> {
  final FocusNode _focusNode = FocusNode(debugLabel: 'shortcut-recorder');
  bool _recording = false;

  @override
  void dispose() {
    if (_recording) widget.onCancelRecording();
    _focusNode.dispose();
    super.dispose();
  }

  Future<void> _startRecording() async {
    if (!await widget.onBeginRecording() || !mounted) return;
    setState(() => _recording = true);
    _focusNode.requestFocus();
  }

  Future<void> _cancelRecording() async {
    if (!_recording) return;
    setState(() => _recording = false);
    await widget.onCancelRecording();
  }

  KeyEventResult _handleKey(FocusNode node, KeyEvent event) {
    if (!_recording || event is! KeyDownEvent) return KeyEventResult.ignored;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      _cancelRecording();
      return KeyEventResult.handled;
    }
    final modifiers = <DesktopShortcutModifier>[
      for (final modifier in DesktopShortcutModifier.values)
        if (modifier.physicalKeys.any(
          HardwareKeyboard.instance.physicalKeysPressed.contains,
        ))
          modifier,
    ];
    final shortcut = DesktopShortcut(
      key: event.physicalKey,
      modifiers: modifiers,
    );
    if (!shortcut.isValid) return KeyEventResult.handled;
    if (widget.accepts?.call(shortcut) == false) {
      widget.onRejected?.call(shortcut);
      return KeyEventResult.handled;
    }
    setState(() => _recording = false);
    widget.onRecorded(shortcut);
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final shortcut = widget.shortcut;
    return Focus(
      focusNode: _focusNode,
      onKeyEvent: _handleKey,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Button.secondary(
            onPressed: !widget.enabled
                ? null
                : _recording
                ? _cancelRecording
                : _startRecording,
            child: _recording
                ? Text(context.l10n.settingsShortcutRecording)
                : shortcut == null
                ? Text(context.l10n.settingsShortcutNotSet)
                : KeyboardDisplay(keys: shortcut.displayKeys),
          ),
          if (widget.onReset case final reset?) ...[
            const Gap(AppSpacing.sm),
            Tooltip(
              showDuration: AppMotion.resolve(context, AppMotion.standard),
              tooltip: (context) => TooltipContainer(
                child: Text(context.l10n.settingsShortcutReset),
              ),
              child: Button.ghost(
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.ghostIcon(),
                ),
                onPressed: !widget.enabled || _recording ? null : reset,
                child: const AppIconFrame(child: Icon(LucideIcons.rotateCcw)),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
