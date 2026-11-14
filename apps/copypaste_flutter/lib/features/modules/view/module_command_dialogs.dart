import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/forms/dynamic_form_draft.dart';
import '../../../shared/forms/dynamic_form_fields.dart';

/// Asks for command arguments; resolves to the filled draft, which the
/// caller closes, or null when cancelled.
Future<DynamicFormDraft?> showModuleArgumentsDialog(
  BuildContext context, {
  required DynamicFormDraft draft,
  required String title,
  required String action,
  Widget? Function(BuildContext context, DynamicFormField field)? fieldBuilder,
}) async {
  final confirmed = await AppOverlays.showDialog<bool>(
    context,
    builder: (_) => _ModuleArgumentsDialog(
      title: title,
      draft: draft,
      action: action,
      fieldBuilder: fieldBuilder,
    ),
  );
  if (confirmed == true) return draft;
  await draft.close();
  return null;
}

/// Shows a command's text result.
Future<void> showModuleResultDialog(
  BuildContext context, {
  required String title,
  required String text,
}) => AppOverlays.showDialog<void>(
  context,
  builder: (dialogContext) => AppOverlays.alertDialog(
    icon: LucideIcons.puzzle,
    title: Text(title),
    content: SingleChildScrollView(child: SelectableText(text)),
    actions: [
      Button.primary(
        style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
        onPressed: () => Navigator.pop(dialogContext),
        child: Text(dialogContext.l10n.commonDone),
      ),
    ],
  ),
);

class _ModuleArgumentsDialog extends StatelessWidget {
  const _ModuleArgumentsDialog({
    required this.title,
    required this.draft,
    required this.action,
    required this.fieldBuilder,
  });
  final String title;
  final DynamicFormDraft draft;
  final String action;
  final Widget? Function(BuildContext context, DynamicFormField field)?
  fieldBuilder;

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: draft,
    builder: (context, _) => AppOverlays.alertDialog(
      icon: LucideIcons.puzzle,
      title: Text(title),
      content: SingleChildScrollView(
        clipBehavior: Clip.none,
        child: DynamicFormFields(draft: draft, fieldBuilder: fieldBuilder),
      ),
      actions: [
        Button.ghost(
          style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
          onPressed: () => Navigator.pop(context, false),
          child: Text(context.l10n.commonCancel),
        ),
        Button.primary(
          style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
          onPressed: draft.valid && !draft.busy
              ? () => Navigator.pop(context, true)
              : null,
          child: Text(action),
        ),
      ],
    ),
  );
}
