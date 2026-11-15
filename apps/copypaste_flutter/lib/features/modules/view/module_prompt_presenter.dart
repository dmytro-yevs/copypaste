import 'dart:async';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../controller/module_prompt_controller.dart';
import '../models/module_prompt.dart';

/// Shows the prompt for [ModulePromptController.current] above [child], one
/// at a time, and closes it when the prompt resolves elsewhere or expires.
/// It must sit below the app's navigator and overlay.
class ModulePromptPresenter extends StatefulWidget {
  const ModulePromptPresenter({
    super.key,
    required this.controller,
    required this.child,
  });

  /// Null until the runtime is ready; the presenter stays in the tree so the
  /// app content keeps its state when prompts become available.
  final ModulePromptController? controller;
  final Widget child;

  @override
  State<ModulePromptPresenter> createState() => _ModulePromptPresenterState();
}

class _ModulePromptPresenterState extends State<ModulePromptPresenter> {
  String? _shown;
  GlobalKey? _dialog;
  bool _closing = false;

  @override
  void initState() {
    super.initState();
    widget.controller?.addListener(_changed);
    WidgetsBinding.instance.addPostFrameCallback((_) => _changed());
  }

  @override
  void didUpdateWidget(ModulePromptPresenter oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller?.removeListener(_changed);
      widget.controller?.addListener(_changed);
      if (widget.controller == null) _closeShown();
      WidgetsBinding.instance.addPostFrameCallback((_) => _changed());
    }
  }

  @override
  void dispose() {
    widget.controller?.removeListener(_changed);
    super.dispose();
  }

  void _closeShown() {
    final dialog = _dialog?.currentContext;
    if (_shown == null || _closing || dialog == null) return;
    _closing = true;
    Navigator.of(dialog).pop();
  }

  void _changed() {
    final controller = widget.controller;
    if (!mounted || controller == null) return;
    final shown = _shown;
    if (shown != null) {
      if (!controller.pending.any((prompt) => prompt.id == shown)) {
        _closeShown();
      }
      return;
    }
    final prompt = controller.current;
    if (prompt != null) unawaited(_present(controller, prompt));
  }

  Future<void> _present(
    ModulePromptController controller,
    ModulePrompt prompt,
  ) async {
    _shown = prompt.id;
    _closing = false;
    final key = _dialog = GlobalKey(debugLabel: 'module-prompt');
    final choice = await AppOverlays.showDialog<String>(
      context,
      barrierDismissible: false,
      builder: (context) => ModulePromptDialog(
        key: key,
        prompt: prompt,
        moduleTitle:
            controller.moduleTitle(prompt) ?? context.l10n.modulesUnknownModule,
      ),
    );
    final closedForUs = _closing;
    _shown = null;
    _dialog = null;
    _closing = false;
    if (!mounted) return;
    if (!closedForUs && !await controller.resolve(prompt, choice) && mounted) {
      AppToast.show(
        context,
        title: context.l10n.appActionFailed,
        message: context.l10n.modulesPromptAnswerFailed,
        tone: AppToastTone.error,
      );
    }
    if (mounted) _changed();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}

/// A module's question with its choices. Escape dismisses it without a
/// choice; it cannot be dismissed otherwise.
class ModulePromptDialog extends StatelessWidget {
  const ModulePromptDialog({
    super.key,
    required this.prompt,
    required this.moduleTitle,
  });

  final ModulePrompt prompt;
  final String moduleTitle;

  @override
  Widget build(BuildContext context) {
    final choices = prompt.choices;
    return PopScope(
      canPop: false,
      child: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): () =>
              Navigator.pop(context),
        },
        child: FocusScope(
          autofocus: true,
          child: AppOverlays.alertDialog(
            icon: LucideIcons.puzzle,
            title: Semantics(
              header: true,
              liveRegion: true,
              child: Text(prompt.title),
            ),
            content: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(context.l10n.modulesPromptFrom(moduleTitle)).small(),
                if (prompt.body.isNotEmpty) ...[
                  const Gap(AppSpacing.sm),
                  Text(prompt.body),
                ],
              ],
            ),
            actions: [
              for (final (index, choice) in choices.indexed)
                Button(
                  key: ValueKey('module-prompt-choice-${choice.id}'),
                  alignment: Alignment.center,
                  style: AppTheme.dialogButtonStyle(
                    choice.destructive
                        ? const ButtonStyle.destructive()
                        : index == choices.length - 1
                        ? const ButtonStyle.primary()
                        : const ButtonStyle.secondary(),
                  ),
                  onPressed: () => Navigator.pop(context, choice.id),
                  child: Text(choice.title),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
