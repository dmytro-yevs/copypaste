import 'dart:async';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../controller/transform_controller.dart';
import '../controller/transforms_settings_controller.dart';
import '../models/transform_models.dart';
import 'transform_param_fields.dart';

/// Creates a pipeline, or edits [pipeline], in the shared drawer.
Future<void> showPipelineEditorDrawer(
  BuildContext context, {
  required TransformsSettingsController controller,
  TransformPipeline? pipeline,
}) => showOverlay<void>(
  context,
  AppOverlays.drawerConfiguration(context),
  builder: (context) =>
      PipelineEditorDrawer(controller: controller, pipeline: pipeline),
).future;

/// A pipeline's name, description and steps. Steps of a transform this
/// build does not know are kept exactly as stored and cannot be edited.
class PipelineEditorDrawer extends StatefulWidget {
  const PipelineEditorDrawer({
    super.key,
    required this.controller,
    this.pipeline,
  });

  final TransformsSettingsController controller;
  final TransformPipeline? pipeline;

  @override
  State<PipelineEditorDrawer> createState() => _PipelineEditorDrawerState();
}

class _PipelineEditorDrawerState extends State<PipelineEditorDrawer> {
  late String _name = widget.pipeline?.name ?? '';
  late String _description = widget.pipeline?.description ?? '';
  late final List<TransformStep> _steps = [...?widget.pipeline?.steps];

  /// Stable keys for step rows, so field state follows a moved step.
  late final List<int> _ids = [for (var i = 0; i < _steps.length; i++) i];
  late int _nextId = _steps.length;
  bool _saving = false;

  TransformsSettingsController get _controller => widget.controller;

  String? _nameProblem(AppLocalizations l10n) {
    final name = _name.trim();
    if (name.isEmpty) return l10n.transformsPipelineNameRequired;
    if (name.length > TransformController.maxPipelineName) {
      return l10n.transformsPipelineNameTooLong;
    }
    if (_controller.pipelineNameTaken(name, exceptId: widget.pipeline?.id)) {
      return l10n.transformsPipelineNameTaken;
    }
    return null;
  }

  bool _valid(AppLocalizations l10n) =>
      _nameProblem(l10n) == null &&
      _steps.isNotEmpty &&
      _steps.length <= TransformLimits.maxSteps;

  void _close() => unawaited(closeDrawer(context));

  Future<void> _save() async {
    if (_saving || !_valid(context.l10n)) return;
    setState(() => _saving = true);
    final saved = await _controller.savePipeline(
      TransformPipelineDraft(
        id: widget.pipeline?.id,
        name: _name.trim(),
        description: _description.trim().isEmpty ? null : _description.trim(),
        steps: List.of(_steps),
      ),
    );
    if (!mounted) return;
    setState(() => _saving = false);
    if (saved) _close();
  }

  void _addStep(String transformId) {
    final definition = _controller.definition(transformId);
    if (definition == null || _steps.length >= TransformLimits.maxSteps) {
      return;
    }
    setState(() {
      _steps.add(
        definition.step(
          _controller.preferences.paramDefaults[transformId] ?? const {},
        ),
      );
      _ids.add(_nextId++);
    });
  }

  void _move(int index, int delta) => setState(() {
    _steps.insert(index + delta, _steps.removeAt(index));
    _ids.insert(index + delta, _ids.removeAt(index));
  });

  void _remove(int index) => setState(() {
    _steps.removeAt(index);
    _ids.removeAt(index);
  });

  @override
  Widget build(BuildContext context) => CallbackShortcuts(
    bindings: {const SingleActivator(LogicalKeyboardKey.escape): _close},
    child: Focus(
      autofocus: true,
      skipTraversal: true,
      child: ListenableBuilder(
        listenable: _controller,
        builder: (context, _) {
          final l10n = context.l10n;
          final nameProblem = _name.isEmpty ? null : _nameProblem(l10n);
          return ConstrainedBox(
            key: const ValueKey('pipeline-editor-drawer'),
            constraints: AppOverlays.drawerContentConstraints(context),
            child: Scaffold(
              headers: [
                AppBar(
                  title: Text(
                    widget.pipeline == null
                        ? l10n.transformsNewPipeline
                        : l10n.transformsEditPipeline,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  leading: const [
                    AppIconFrame(child: Icon(LucideIcons.workflow)),
                  ],
                  trailing: [
                    Tooltip(
                      tooltip: (context) =>
                          TooltipContainer(child: Text(l10n.commonClose)),
                      child: Button.ghost(
                        style: AppTheme.controlButtonStyle(
                          const ButtonStyle.ghostIcon(),
                        ),
                        onPressed: _close,
                        child: Semantics(
                          label: l10n.commonClose,
                          child: const AppIconFrame(child: Icon(LucideIcons.x)),
                        ),
                      ),
                    ),
                  ],
                ),
                const Divider(),
              ],
              footers: [
                const Divider(),
                Padding(
                  padding: const EdgeInsets.all(AppSpacing.lg),
                  child: Row(
                    children: [
                      Expanded(
                        child: Button.secondary(
                          style: AppTheme.actionButtonStyle(
                            const ButtonStyle.secondary(),
                          ),
                          alignment: Alignment.center,
                          onPressed: _saving ? null : _close,
                          child: Text(l10n.commonCancel),
                        ),
                      ),
                      const Gap(AppSpacing.md),
                      Expanded(
                        child: Button.primary(
                          key: const ValueKey('pipeline-editor-save'),
                          style: AppTheme.actionButtonStyle(
                            const ButtonStyle.primary(),
                          ),
                          alignment: Alignment.center,
                          onPressed: _saving || !_valid(l10n) ? null : _save,
                          child: Text(l10n.commonSave),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(AppSpacing.lg),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (_controller.errorMessage case final error?) ...[
                      StateView.error(
                        title: l10n.transformsSavePipelineFailed,
                        message: error,
                      ),
                      const Gap(AppSpacing.md),
                    ],
                    Text(l10n.transformsPipelineNameLabel).small(),
                    const Gap(AppSpacing.xs),
                    TextField(
                      key: const ValueKey('pipeline-editor-name'),
                      initialValue: _name,
                      maxLength: TransformController.maxPipelineName,
                      decoration: AppOverlays.dialogFieldDecoration(context),
                      onChanged: (value) => setState(() => _name = value),
                    ),
                    if (nameProblem != null) ...[
                      const Gap(AppSpacing.xs),
                      Text(nameProblem).small().muted(),
                    ],
                    const Gap(AppSpacing.md),
                    Text(l10n.transformsPipelineDescriptionLabel).small(),
                    const Gap(AppSpacing.xs),
                    TextField(
                      initialValue: _description,
                      decoration: AppOverlays.dialogFieldDecoration(context),
                      onChanged: (value) => _description = value,
                    ),
                    const Gap(AppSpacing.lg),
                    Text(l10n.transformsPipelineStepsLabel).medium(),
                    const Gap(AppSpacing.sm),
                    if (_steps.isEmpty)
                      StateView.empty(
                        compact: true,
                        icon: LucideIcons.workflow,
                        title: l10n.transformsPipelineNoSteps,
                      ),
                    for (var index = 0; index < _steps.length; index++) ...[
                      _stepCard(context, index),
                      const Gap(AppSpacing.sm),
                    ],
                    if (_steps.length < TransformLimits.maxSteps)
                      Select<String>(
                        key: const ValueKey('pipeline-editor-add-step'),
                        placeholder: Text(l10n.transformsAddStep),
                        onChanged: (id) {
                          if (id != null) _addStep(id);
                        },
                        itemBuilder: (context, id) =>
                            Text(_controller.definition(id)?.name ?? id),
                        popup: SelectPopup<String>(
                          items: SelectItemList(
                            children: [
                              for (final definition in _controller.catalogue)
                                SelectItemButton<String>(
                                  value: definition.id,
                                  child: Text(definition.name),
                                ),
                            ],
                          ),
                        ).call,
                      ),
                  ],
                ),
              ),
            ),
          );
        },
      ),
    ),
  );

  Widget _stepCard(BuildContext context, int index) {
    final l10n = context.l10n;
    final step = _steps[index];
    final definition = _controller.definition(step.transformId);
    final known = definition != null && step.version <= definition.version;
    return Card(
      key: ValueKey('pipeline-step-${_ids[index]}'),
      theme: AppTheme.settingsGroupCardTheme,
      child: Padding(
        padding: const EdgeInsets.all(AppSpacing.md),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    l10n.transformsStepLabel(
                      index + 1,
                      definition?.name ?? step.transformId,
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ).medium(),
                ),
                Semantics(
                  label: l10n.transformsStepEnabled,
                  child: Switch(
                    value: step.enabled,
                    onChanged: (value) => setState(
                      () => _steps[index] = step.copyWith(enabled: value),
                    ),
                  ),
                ),
                _iconButton(
                  l10n.transformsMoveEarlier,
                  LucideIcons.arrowUp,
                  index > 0 ? () => _move(index, -1) : null,
                ),
                _iconButton(
                  l10n.transformsMoveLater,
                  LucideIcons.arrowDown,
                  index < _steps.length - 1 ? () => _move(index, 1) : null,
                ),
                _iconButton(
                  l10n.transformsRemoveStep,
                  LucideIcons.x,
                  () => _remove(index),
                ),
              ],
            ),
            if (!known) ...[
              const Gap(AppSpacing.xs),
              Text(l10n.transformsNeedsNewerVersion).small().muted(),
            ] else if (definition.params.isNotEmpty) ...[
              const Gap(AppSpacing.sm),
              TransformParamFields(
                keyPrefix: 'pipeline-step-${_ids[index]}',
                params: definition.params,
                valueOf: (param) => step.params.containsKey(param.key)
                    ? step.params[param.key]
                    : param.defaultValue,
                onChanged: (param, value) {
                  if (!param.accepts(value)) return;
                  setState(
                    () => _steps[index] = _steps[index].copyWith(
                      params: {..._steps[index].params, param.key: value},
                    ),
                  );
                },
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _iconButton(String label, IconData icon, VoidCallback? onPressed) =>
      Tooltip(
        tooltip: (context) => TooltipContainer(child: Text(label)),
        child: Button.ghost(
          style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
          onPressed: onPressed,
          child: Semantics(
            label: label,
            child: AppIconFrame(child: Icon(icon)),
          ),
        ),
      );
}
