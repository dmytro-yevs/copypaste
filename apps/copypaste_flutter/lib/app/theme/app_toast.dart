import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'app_theme.dart';

enum AppToastTone { information, success, error }

/// The single application-owned toast component.
///
/// Every toast uses this component so presentation and behavior remain
/// consistent across macOS, Android, and Windows.
class AppToast extends StatelessWidget {
  const AppToast({
    super.key,
    required this.title,
    required this.message,
    this.tone = AppToastTone.information,
    this.actionLabel,
    this.onAction,
  }) : assert((actionLabel == null) == (onAction == null));

  final String title;
  final String message;
  final AppToastTone tone;

  /// One action, such as Undo, that also closes the toast.
  final String? actionLabel;
  final VoidCallback? onAction;

  /// A toast with an action stays twice as long while assistive technology
  /// is active, so the action can still be reached.
  static ToastOverlay show(
    BuildContext context, {
    required String title,
    required String message,
    AppToastTone tone = AppToastTone.information,
    String? actionLabel,
    VoidCallback? onAction,
  }) {
    final accessible =
        actionLabel != null && MediaQuery.maybeAccessibleNavigationOf(context) == true;
    return showToast(
      context: context,
      showDuration: Duration(seconds: accessible ? 10 : 5),
      builder: (context, overlay) => AppToast(
        title: title,
        message: message,
        tone: tone,
        actionLabel: actionLabel,
        onAction: onAction == null
            ? null
            : () {
                overlay.close();
                onAction();
              },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final icon = switch (tone) {
      AppToastTone.information => LucideIcons.info,
      AppToastTone.success => LucideIcons.circleCheck,
      AppToastTone.error => LucideIcons.circleAlert,
    };
    final content = Text(message);
    final action = actionLabel == null
        ? null
        : Button.ghost(
            style: AppTheme.controlButtonStyle(const ButtonStyle.ghost()),
            onPressed: onAction,
            child: Text(actionLabel!),
          );
    if (tone == AppToastTone.error) {
      return Alert.destructive(
        leading: AppIconFrame(child: Icon(icon)),
        title: Text(title),
        content: content,
        trailing: action,
      );
    }
    return Alert(
      leading: AppIconFrame(child: Icon(icon)),
      title: Text(title),
      content: content,
      trailing: action,
    );
  }
}
