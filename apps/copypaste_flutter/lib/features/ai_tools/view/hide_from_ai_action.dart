import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../l10n/l10n.dart';

/// The per-item "Hide from AI tools" / "Show to AI tools" toggle used by
/// item surfaces. [onChanged] receives the requested hidden state; the
/// caller writes it and refreshes [hidden]. Un-hiding is accepted only
/// from the CopyPaste app.
class HideFromAiAction extends StatelessWidget {
  const HideFromAiAction({
    super.key,
    required this.hidden,
    required this.onChanged,
    this.iconOnly = false,
  });

  final bool hidden;
  final ValueChanged<bool>? onChanged;
  final bool iconOnly;

  /// The action label for an item in [hidden] state, for menus too.
  static String label(BuildContext context, {required bool hidden}) =>
      hidden ? context.l10n.aiToolsShow : context.l10n.aiToolsHide;

  /// The action icon for an item in [hidden] state, for menus too.
  static IconData icon({required bool hidden}) =>
      hidden ? LucideIcons.eye : LucideIcons.eyeOff;

  @override
  Widget build(BuildContext context) {
    final text = label(context, hidden: hidden);
    final glyph = AppIconFrame(child: Icon(icon(hidden: hidden)));
    return Tooltip(
      tooltip: (_) => TooltipContainer(child: Text(text)),
      child: Semantics(
        label: text,
        toggled: hidden,
        value: hidden ? context.l10n.aiToolsHiddenState : null,
        child: Button.outline(
          key: const ValueKey('hide-from-ai'),
          style: AppTheme.actionButtonStyle(
            iconOnly
                ? const ButtonStyle.outlineIcon()
                : const ButtonStyle.outline(),
          ),
          onPressed: onChanged == null ? null : () => onChanged!(!hidden),
          leading: iconOnly ? null : glyph,
          child: iconOnly ? glyph : Text(text),
        ),
      ),
    );
  }
}
