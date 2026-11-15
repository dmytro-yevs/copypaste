import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/forms/dynamic_form_draft.dart';
import '../../../shared/forms/dynamic_form_fields.dart';
import '../../../shared/forms/placeholder_text_controller.dart';
import '../models/module_view.dart';

/// A module form: its fields, the module's field errors, the preview its
/// preview action renders after edits, and the submit button. Key it by the
/// form, so a new form from the module starts from its values.
class ModuleViewFormCard extends StatefulWidget {
  const ModuleViewFormCard({
    super.key,
    required this.form,
    required this.onSubmit,
    this.onPreview,
    this.createDraft,
    this.fieldBuilder,
    this.enabled = true,
    this.autofocus = false,
  });

  final ModuleViewForm form;

  /// Receives the submit action with the field values as `values`.
  final ValueChanged<ModuleViewAction> onSubmit;

  /// Runs the preview action with the field values; resolves to its text.
  final Future<String?> Function(ModuleViewAction preview)? onPreview;
  final DynamicFormDraft Function(
    List<DynamicFormField> fields,
    Map<String, Object> values,
  )?
  createDraft;
  final Widget? Function(DynamicFormDraft draft, DynamicFormField field)?
  fieldBuilder;
  final bool enabled;
  final bool autofocus;

  @override
  State<ModuleViewFormCard> createState() => _ModuleViewFormCardState();
}

class _ModuleViewFormCardState extends State<ModuleViewFormCard> {
  static const _previewDelay = Duration(milliseconds: 300);

  late final DynamicFormDraft _draft =
      widget.createDraft?.call(widget.form.fields, widget.form.values) ??
      DynamicFormDraft(fields: widget.form.fields, initial: widget.form.values);
  late final Map<String, TextEditingController> _texts = {
    for (final field in widget.form.fields)
      if (field.template)
        field.id: PlaceholderTextController(
          text: _draft.values[field.id] as String? ?? '',
        ),
  };
  late final Map<String, FocusNode> _focus = {
    for (final field in widget.form.fields)
      field.id: FocusNode(debugLabel: 'module-form-${field.id}'),
  };
  Timer? _previewTimer;
  String? _preview;
  int _previewRequest = 0;

  @override
  void initState() {
    super.initState();
    _draft.addListener(_changed);
    if (widget.form.preview != null) unawaited(_renderPreview());
    if (widget.autofocus) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _focus.values.firstOrNull?.requestFocus();
      });
    }
  }

  void _changed() {
    if (widget.form.preview == null) return;
    _previewTimer?.cancel();
    _previewTimer = Timer(_previewDelay, () => unawaited(_renderPreview()));
  }

  Future<void> _renderPreview() async {
    final preview = widget.form.preview;
    final render = widget.onPreview;
    if (preview == null || render == null) return;
    final request = ++_previewRequest;
    final text = await render(preview.withArguments({'values': _draft.values}));
    if (mounted && request == _previewRequest) setState(() => _preview = text);
  }

  void _submit() {
    if (!widget.enabled || !_draft.valid || _draft.busy) return;
    widget.onSubmit(
      widget.form.submit.withArguments({'values': _draft.values}),
    );
  }

  @override
  void dispose() {
    _previewTimer?.cancel();
    _draft.removeListener(_changed);
    unawaited(_draft.close());
    for (final controller in _texts.values) {
      controller.dispose();
    }
    for (final node in _focus.values) {
      node.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final mod = defaultTargetPlatform == TargetPlatform.macOS;
    final last = widget.form.fields.lastOrNull?.id;
    return CallbackShortcuts(
      bindings: {
        SingleActivator(LogicalKeyboardKey.enter, meta: mod, control: !mod):
            _submit,
      },
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          DynamicFormFields(
            draft: _draft,
            enabled: widget.enabled,
            focusNodes: _focus,
            textControllers: _texts,
            errors: widget.form.errors,
            onSubmitted: (_) {
              if (_focus[last]?.hasFocus ?? false) {
                _submit();
              } else {
                FocusManager.instance.primaryFocus?.nextFocus();
              }
            },
            fieldBuilder: widget.fieldBuilder == null
                ? null
                : (context, field) => widget.fieldBuilder!(_draft, field),
          ),
          if (_preview case final preview? when preview.isNotEmpty) ...[
            Card(
              theme: AppTheme.settingsGroupCardTheme,
              child: Padding(
                padding: const EdgeInsets.all(AppSpacing.md),
                child: Text(
                  preview,
                  key: const ValueKey('module-form-preview'),
                ),
              ),
            ),
            const Gap(AppSpacing.md),
          ],
          ListenableBuilder(
            listenable: _draft,
            builder: (context, _) => Button.primary(
              key: ValueKey('module-form-submit-${widget.form.submit.command}'),
              style: AppTheme.actionButtonStyle(const ButtonStyle.primary()),
              alignment: AppTheme.moduleSettingsActionAlignment,
              onPressed: widget.enabled && _draft.valid && !_draft.busy
                  ? _submit
                  : null,
              child: Text(
                widget.form.submit.title,
                textAlign: TextAlign.center,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
