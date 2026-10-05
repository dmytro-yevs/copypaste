import 'package:shadcn_flutter/shadcn_flutter.dart';

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
  });

  final String title;
  final String message;
  final AppToastTone tone;

  static ToastOverlay show(
    BuildContext context, {
    required String title,
    required String message,
    AppToastTone tone = AppToastTone.information,
  }) {
    return showToast(
      context: context,
      builder: (context, overlay) =>
          AppToast(title: title, message: message, tone: tone),
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
    if (tone == AppToastTone.error) {
      return Alert.destructive(
        leading: Icon(icon),
        title: Text(title),
        content: content,
      );
    }
    return Alert(leading: Icon(icon), title: Text(title), content: content);
  }
}
