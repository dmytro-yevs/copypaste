import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../../l10n/l10n.dart';
import '../../../shared/forms/dynamic_form_draft.dart';
import '../models/snippet_models.dart';
import '../repository/snippet_repository.dart';

/// The fill-in form of one template use. Field values live only in memory
/// and are cleared when the form closes. The preview is rendered again after
/// each change, with no timer; changes made while a render runs are folded
/// into one more render.
class TemplateFillController extends ChangeNotifier {
  TemplateFillController({
    required SnippetRepository repository,
    required this.snippet,
    this.clipboard = TemplateClipboardSource.host,
  }) : _repository = repository,
       draft = DynamicFormDraft(
         fields: [
           for (final field in snippet.fields)
             DynamicFormField(
               id: field.name,
               title: field.label,
               kind: switch (field.kind) {
                 TemplateFieldKind.text => DynamicFormFieldKind.line,
                 TemplateFieldKind.multiline => DynamicFormFieldKind.text,
                 TemplateFieldKind.choice => DynamicFormFieldKind.choice,
               },
               defaultValue: field.initialValue,
               required: field.required,
               options: [
                 for (final option in field.options)
                   DynamicFormChoice(id: option, title: option),
               ],
             ),
         ],
         initial: const {},
       ) {
    for (final field in snippet.fields) {
      _focus[field.name] = FocusNode(debugLabel: 'template-${field.name}');
    }
    draft.addListener(_changed);
    unawaited(_render());
  }

  final SnippetRepository _repository;
  final SnippetDetail snippet;
  final TemplateClipboardSource clipboard;
  final DynamicFormDraft draft;
  final Map<String, FocusNode> _focus = {};
  Map<String, String> _errors = const {};
  RenderedTemplate? _rendered;
  bool _rendering = false;
  bool _renderAgain = false;
  bool _submitting = false;
  String? _error;
  bool _disposed = false;

  Map<String, FocusNode> get focusNodes => Map.unmodifiable(_focus);
  Map<String, String> get errors => _errors;
  RenderedTemplate? get rendered => _rendered;
  bool get submitting => _submitting;
  String? get errorMessage => _error;

  Map<String, String> get values => {
    for (final MapEntry(:key, :value) in draft.values.entries)
      key: value is String ? value : '',
  };

  /// Warnings shown above the preview: placeholders left verbatim and empty
  /// sources.
  List<TemplateLint> get warnings => _rendered?.warnings ?? const [];

  int get unfilled => _rendered?.unfilled ?? 0;

  void _changed() {
    if (_errors.isNotEmpty) {
      _errors = {
        for (final MapEntry(:key, :value) in _errors.entries)
          if ((values[key] ?? '').trim().isEmpty) key: value,
      };
    }
    unawaited(_render());
    notifyListeners();
  }

  Future<void> _render() async {
    if (_rendering) {
      _renderAgain = true;
      return;
    }
    _rendering = true;
    try {
      do {
        _renderAgain = false;
        final rendered = await _repository.render(
          snippetId: snippet.id,
          values: values,
          clipboard: clipboard,
        );
        if (_disposed) return;
        _rendered = rendered;
        notifyListeners();
      } while (_renderAgain && !_disposed);
    } on Object {
      // The preview keeps its last result; using the snippet reports errors.
    } finally {
      _rendering = false;
    }
  }

  /// Marks empty required fields and focuses the first one.
  bool validate() {
    final message = AppStrings.current.snippetsFieldRequired;
    final missing = [
      for (final field in snippet.fields)
        if (field.required && (values[field.name] ?? '').trim().isEmpty)
          field.name,
    ];
    _errors = {for (final name in missing) name: message};
    notifyListeners();
    if (missing.isNotEmpty) _focus[missing.first]?.requestFocus();
    return missing.isEmpty;
  }

  /// Uses the snippet with the entered values, unless a required field is
  /// empty or the runtime refuses (for example a required clipboard value).
  Future<SnippetDelivered?> submit(SnippetDelivery delivery) async {
    if (_submitting || !validate()) return null;
    _submitting = true;
    _error = null;
    notifyListeners();
    try {
      return await _repository.use(
        snippet.id,
        values: values,
        delivery: delivery,
      );
    } on Object catch (error) {
      _error = error is SnippetException
          ? error.message
          : AppStrings.current.snippetsUseFailed;
      return null;
    } finally {
      _submitting = false;
      notifyListeners();
    }
  }

  @override
  void notifyListeners() {
    if (!_disposed) super.notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    draft.removeListener(_changed);
    unawaited(draft.close());
    for (final node in _focus.values) {
      node.dispose();
    }
    super.dispose();
  }
}
