import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../app/theme/app_tokens.dart';

enum StateViewKind { loading, empty, error }

/// The application's single state presentation for loading, empty, and errors.
class StateView extends StatelessWidget {
  const StateView.loading({super.key, this.message})
    : kind = StateViewKind.loading,
      title = 'Loading',
      actionLabel = null,
      onAction = null;

  const StateView.empty({
    super.key,
    required this.title,
    this.message,
    this.actionLabel,
    this.onAction,
  }) : kind = StateViewKind.empty,
       assert((actionLabel == null) == (onAction == null));

  const StateView.error({
    super.key,
    required this.title,
    this.message,
    this.actionLabel,
    this.onAction,
  }) : kind = StateViewKind.error,
       assert((actionLabel == null) == (onAction == null));

  final StateViewKind kind;
  final String title;
  final String? message;
  final String? actionLabel;
  final VoidCallback? onAction;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final content = _contentFor(constraints.maxWidth);
        final child = ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 480),
          child: content,
        );
        final paddedChild = Padding(
          padding: const EdgeInsets.all(AppSpacing.xxl),
          child: Center(child: child),
        );
        if (!constraints.hasBoundedHeight) return paddedChild;
        return SingleChildScrollView(
          child: ConstrainedBox(
            constraints: BoxConstraints(minHeight: constraints.maxHeight),
            child: paddedChild,
          ),
        );
      },
    );
  }

  Widget _contentFor(double availableWidth) {
    return switch (kind) {
      StateViewKind.loading => _standardContent(
        leading: const CircularProgressIndicator(),
      ),
      StateViewKind.empty => _standardContent(
        leading: const Icon(LucideIcons.inbox, size: AppIconSize.state),
      ),
      StateViewKind.error => _errorContent(
        showTrailingAction: availableWidth >= 420,
      ),
    };
  }

  Widget _errorContent({required bool showTrailingAction}) {
    final action = actionLabel == null
        ? null
        : Button.primary(
            onPressed: onAction,
            child: Text(actionLabel!, textAlign: TextAlign.center),
          );
    return Alert.destructive(
      leading: const Icon(LucideIcons.circleAlert),
      title: Text(title),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (message != null) Text(message!),
          if (action != null && !showTrailingAction) ...[
            const Gap(AppSpacing.lg),
            SizedBox(width: double.infinity, child: action),
          ],
        ],
      ),
      trailing: showTrailingAction ? action : null,
    );
  }

  Widget _standardContent({required Widget leading}) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        ExcludeSemantics(child: leading),
        const Gap(AppSpacing.lg),
        Text(title, textAlign: TextAlign.center).h3(),
        if (message != null) ...[
          const Gap(AppSpacing.sm),
          Text(message!, textAlign: TextAlign.center).muted(),
        ],
        if (actionLabel != null) ...[
          const Gap(AppSpacing.lg),
          Button.primary(
            onPressed: onAction,
            child: Text(actionLabel!, textAlign: TextAlign.center),
          ),
        ],
      ],
    );
  }
}
