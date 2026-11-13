import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/forms/dynamic_form_fields.dart';
import '../controller/template_fill_controller.dart';
import '../models/snippet_models.dart';
import '../presentation/snippet_messages.dart';

/// The fields of a template in body order, the warnings and the live
/// preview. Enter in the last single-line field or Mod+Enter anywhere calls
/// [onSubmit]; owners place their own actions, such as Paste or Copy.
class TemplateFillForm extends StatelessWidget {
  const TemplateFillForm({
    super.key,
    required this.controller,
    required this.onSubmit,
  });

  final TemplateFillController controller;
  final VoidCallback onSubmit;

  @override
  Widget build(BuildContext context) {
    final mod = defaultTargetPlatform == TargetPlatform.macOS;
    final last = controller.snippet.fields.lastOrNull?.name;
    return CallbackShortcuts(
      bindings: {
        SingleActivator(LogicalKeyboardKey.enter, meta: mod, control: !mod):
            onSubmit,
      },
      child: ListenableBuilder(
        listenable: controller,
        builder: (context, _) {
          final l10n = context.l10n;
          final rendered = controller.rendered;
          final warnings = controller.warnings;
          return Column(
            key: const ValueKey('template-fill-form'),
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (controller.errorMessage case final error?) ...[
                Alert.destructive(
                  leading: const AppIconFrame(
                    child: Icon(LucideIcons.circleAlert),
                  ),
                  title: Text(l10n.snippetsUseFailed),
                  content: Text(error),
                ),
                const Gap(AppSpacing.md),
              ],
              DynamicFormFields(
                draft: controller.draft,
                enabled: !controller.submitting,
                focusNodes: controller.focusNodes,
                errors: controller.errors,
                onSubmitted: (_) {
                  if (controller.focusNodes[last]?.hasFocus ?? false) {
                    onSubmit();
                  } else {
                    FocusManager.instance.primaryFocus?.nextFocus();
                  }
                },
              ),
              if (warnings.isNotEmpty || controller.unfilled > 0) ...[
                Semantics(
                  liveRegion: true,
                  child: Column(
                    key: const ValueKey('template-fill-warnings'),
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      if (controller.unfilled > 0)
                        Text(
                          l10n.snippetsUnfilledWarning(controller.unfilled),
                        ).small().muted(),
                      for (final warning in warnings)
                        Text(l10n.snippetLint(warning)).small().muted(),
                    ],
                  ),
                ),
                const Gap(AppSpacing.sm),
              ],
              Text(l10n.snippetsPreviewTab).small(),
              const Gap(AppSpacing.xs),
              Card(
                theme: AppTheme.settingsGroupCardTheme,
                child: Padding(
                  padding: const EdgeInsets.all(AppSpacing.md),
                  child: Text(
                    rendered == null || rendered.text.isEmpty
                        ? l10n.snippetsPreviewEmpty
                        : rendered.text,
                    key: const ValueKey('template-fill-preview'),
                  ),
                ),
              ),
            ],
          );
        },
      ),
    );
  }
}

/// Fills [snippet]'s fields and copies the result, in the shared drawer
/// with Back and Copy in a fixed footer. Resolves to true once copied.
Future<bool> showTemplateFillDrawer(
  BuildContext context, {
  required TemplateFillController controller,
}) async {
  final copied = await showOverlay<bool>(
    context,
    AppOverlays.drawerConfiguration(context),
    builder: (context) => _TemplateFillDrawer(controller: controller),
  ).future;
  return copied ?? false;
}

class _TemplateFillDrawer extends StatelessWidget {
  const _TemplateFillDrawer({required this.controller});

  final TemplateFillController controller;

  Future<void> _copy(BuildContext context) async {
    final delivered = await controller.submit(SnippetDelivery.copy);
    if (delivered != null && context.mounted) {
      await closeDrawer(context, true);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final title = l10n.snippetTitle(controller.snippet.summary);
    return AppOverlays.drawerDismissal(
      onDismiss: () => unawaited(closeDrawer(context, false)),
      child: Focus(
        skipTraversal: true,
        child: ConstrainedBox(
          key: const ValueKey('template-fill-drawer'),
          constraints: AppOverlays.drawerContentConstraints(context),
          child: Scaffold(
            headers: [
              AppBar(
                title: Text(
                  title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                leading: const [AppIconFrame(child: Icon(LucideIcons.braces))],
              ),
              const Divider(),
            ],
            footers: [
              const Divider(),
              Padding(
                padding: const EdgeInsets.all(AppSpacing.lg),
                child: ListenableBuilder(
                  listenable: controller,
                  builder: (context, _) => Row(
                    children: [
                      Expanded(
                        child: Button.secondary(
                          style: AppTheme.actionButtonStyle(
                            const ButtonStyle.secondary(),
                          ),
                          alignment: Alignment.center,
                          onPressed: () =>
                              unawaited(closeDrawer(context, false)),
                          child: Text(l10n.commonBack),
                        ),
                      ),
                      const Gap(AppSpacing.md),
                      Expanded(
                        child: Button.primary(
                          key: const ValueKey('template-fill-copy'),
                          style: AppTheme.actionButtonStyle(
                            const ButtonStyle.primary(),
                          ),
                          alignment: Alignment.center,
                          onPressed: controller.submitting
                              ? null
                              : () => unawaited(_copy(context)),
                          child: Text(l10n.commonCopy),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ],
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(AppSpacing.lg),
              child: TemplateFillForm(
                controller: controller,
                onSubmit: () => unawaited(_copy(context)),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
