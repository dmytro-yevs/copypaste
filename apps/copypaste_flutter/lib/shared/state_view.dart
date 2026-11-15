import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../app/theme/app_theme.dart';
import '../app/theme/app_tokens.dart';
import '../l10n/l10n.dart';

enum StateViewKind { loading, empty, error }

/// The application's single state presentation for loading, empty, and errors.
class StateView extends StatelessWidget {
  const StateView.loading({super.key, this.message})
    : kind = StateViewKind.loading,
      title = null,
      compact = false,
      icon = LucideIcons.inbox,
      actionLabel = null,
      onAction = null,
      _retry = false;

  const StateView.empty({
    super.key,
    required String this.title,
    this.message,
    this.compact = false,
    this.icon = LucideIcons.inbox,
    this.actionLabel,
    this.onAction,
  }) : kind = StateViewKind.empty,
       _retry = false,
       assert((actionLabel == null) == (onAction == null));

  /// A view or section that failed to load, shown in place of its content.
  ///
  /// [title] says what failed; [message] is an optional, different line on
  /// why (see `ErrorText.reason`). [onRetry] offers the standard "Try
  /// again"; [actionLabel] with [onAction] offers any other recovery.
  const StateView.error({
    super.key,
    required String this.title,
    this.message,
    VoidCallback? onRetry,
    String? actionLabel,
    VoidCallback? onAction,
  }) : kind = StateViewKind.error,
       compact = false,
       icon = LucideIcons.circleAlert,
       _retry = onRetry != null,
       actionLabel = actionLabel,
       onAction = onRetry ?? onAction,
       assert(onRetry == null || actionLabel == null),
       assert(onRetry != null || (actionLabel == null) == (onAction == null));

  final StateViewKind kind;
  final bool compact;
  final IconData icon;
  final String? title;
  final String? message;
  final String? actionLabel;
  final VoidCallback? onAction;
  final bool _retry;

  @override
  Widget build(BuildContext context) {
    final title = this.title ?? context.l10n.commonLoading;
    final content = _contentFor(
      context,
      title,
      _retry ? context.l10n.commonTryAgain : actionLabel,
    );
    return LayoutBuilder(
      builder: (context, constraints) {
        final child = ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 480),
          child: content,
        );
        final paddedChild = Padding(
          padding: EdgeInsets.all(compact ? AppSpacing.lg : AppSpacing.xxl),
          child: Center(child: child),
        );
        if (!constraints.hasBoundedHeight) return paddedChild;
        final bottomPadding = MediaQuery.paddingOf(context).bottom;
        return SingleChildScrollView(
          padding: EdgeInsets.only(bottom: bottomPadding),
          child: ConstrainedBox(
            constraints: BoxConstraints(
              minHeight: (constraints.maxHeight - bottomPadding).clamp(
                0.0,
                double.infinity,
              ),
            ),
            child: paddedChild,
          ),
        );
      },
    );
  }

  Widget _contentFor(BuildContext context, String title, String? action) {
    return switch (kind) {
      StateViewKind.loading => _standardContent(
        title: title,
        action: action,
        leading: const AppIconFrame(
          size: AppIconSize.sm,
          child: CircularProgressIndicator(),
        ),
      ),
      StateViewKind.empty => _standardContent(
        title: title,
        action: action,
        leading: AppIconFrame(
          size: compact ? AppIconSize.lg : AppIconSize.state,
          child: Icon(icon, size: compact ? AppIconSize.lg : AppIconSize.state),
        ),
      ),
      StateViewKind.error => _standardContent(
        title: title,
        action: action,
        leading: AppIconFrame(
          size: AppIconSize.state,
          child: Icon(
            icon,
            size: AppIconSize.state,
            color: AppTheme.statusColor(context, AppStatusTone.error),
          ),
        ),
      ),
    };
  }

  Widget _standardContent({
    required String title,
    required Widget leading,
    required String? action,
  }) {
    final message = this.message;
    final showMessage =
        message != null && message.trim().isNotEmpty && message != title;
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        ExcludeSemantics(child: leading),
        Gap(compact ? AppSpacing.md : AppSpacing.lg),
        if (compact)
          Text(title, textAlign: TextAlign.center).small().muted()
        else
          Text(title, textAlign: TextAlign.center).h3(),
        if (showMessage) ...[
          const Gap(AppSpacing.sm),
          Text(message, textAlign: TextAlign.center).muted(),
        ],
        if (action != null) ...[
          const Gap(AppSpacing.lg),
          Button.primary(
            onPressed: onAction,
            child: Text(action, textAlign: TextAlign.center),
          ),
        ],
      ],
    );
  }
}
