import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../../l10n/l10n.dart';
import '../error_text.dart';

/// [text] is multi-line; [line] is a single line; [choices] picks several
/// options and [choice] exactly one. [devices] and [applications] hold ids
/// chosen from pickers that the form's owner renders. [ids] and [formValues]
/// are filled by the host, from a selection or a form, and never shown.
enum DynamicFormFieldKind {
  text,
  line,
  boolean,
  file,
  choices,
  choice,
  devices,
  applications,
  ids,
  formValues;

  bool get hostFilled => this == ids || this == formValues;
}

class DynamicFormChoice {
  const DynamicFormChoice({required this.id, required this.title});
  final String id;
  final String title;
}

/// One field of a form whose fields are described at run time.
class DynamicFormField {
  const DynamicFormField({
    required this.id,
    required this.title,
    required this.kind,
    required this.defaultValue,
    this.required = false,
    this.secret = false,
    this.options = const [],
    this.acceptedExtensions = const [],
    this.maxBytes = 64 * 1024 * 1024,
    this.template = false,
  });
  final String id;
  final String title;
  final DynamicFormFieldKind kind;
  final Object defaultValue;
  final bool required;
  final bool secret;
  final List<DynamicFormChoice> options;
  final List<String> acceptedExtensions;

  /// The largest file, or the most UTF-8 bytes of a text value.
  final int maxBytes;

  /// Text whose `{{…}}` placeholders are highlighted.
  final bool template;
}

abstract interface class DynamicFormInputPicker {
  Future<SelectedDynamicFormInput?> chooseInput(DynamicFormField field);
}

/// A selected input file whose lease ends with [dispose].
class SelectedDynamicFormInput {
  const SelectedDynamicFormInput({
    required this.path,
    required this.name,
    required this.dispose,
  });
  final String path;
  final String name;
  final Future<void> Function() dispose;
}

/// Owns transient form values and selected input-file leases until invocation ends.
class DynamicFormDraft extends ChangeNotifier {
  DynamicFormDraft({
    required this.fields,
    required Map<String, Object> initial,
    DynamicFormInputPicker? picker,
    bool Function(Map<String, Object> values)? accepts,
  }) : _picker = picker,
       _accepts = accepts,
       _values = {
         for (final field in fields)
           field.id: initial[field.id] ?? field.defaultValue,
       };
  final List<DynamicFormField> fields;

  final DynamicFormInputPicker? _picker;

  /// Owner validation across fields, applied after per-field validation.
  final bool Function(Map<String, Object> values)? _accepts;
  final Map<String, Object> _values;
  final Map<String, SelectedDynamicFormInput> _files = {};
  bool _busy = false;
  bool _closed = false;
  String? _error;
  Map<String, Object> get values => Map.unmodifiable(_values);
  bool get busy => _busy;
  String? get errorMessage => _error;
  String? fileName(String id) => _files[id]?.name;
  bool get valid =>
      fields.every((field) {
        final value = _values[field.id];
        if (field.kind.hostFilled) return true;
        if (field.kind
            case DynamicFormFieldKind.text || DynamicFormFieldKind.line
            when value is String &&
                value.length * 3 > field.maxBytes &&
                utf8.encode(value).length > field.maxBytes) {
          return false;
        }
        if (field.kind
            case DynamicFormFieldKind.choices ||
                DynamicFormFieldKind.devices ||
                DynamicFormFieldKind.applications) {
          if (value is! List || (field.required && value.isEmpty)) return false;
          return value.length == value.toSet().length &&
              (field.kind != DynamicFormFieldKind.choices ||
                  value.every(
                    (id) => field.options.any((option) => option.id == id),
                  ));
        }
        return !field.required ||
            field.kind == DynamicFormFieldKind.boolean ||
            (value is String && value.trim().isNotEmpty);
      }) &&
      (_accepts?.call(values) ?? true);

  void setValue(String id, Object value) {
    if (_closed) return;
    _values[id] = value;
    notifyListeners();
  }

  Future<void> chooseFile(DynamicFormField field) async {
    if (_busy || _closed) return;
    _busy = true;
    _error = null;
    notifyListeners();
    try {
      final picker = _picker;
      if (picker == null) {
        _error = AppStrings.current.modulesFileSelectionUnavailable;
        return;
      }
      final file = await picker.chooseInput(field);
      if (file == null) return;
      if (_closed) {
        await file.dispose();
        return;
      }
      final previous = _files[field.id];
      _files[field.id] = file;
      _values[field.id] = file.path;
      await previous?.dispose();
    } catch (error) {
      if (!_closed) {
        _error =
            ErrorText.reason(error) ??
            AppStrings.current.modulesFileSelectFailed;
      }
    } finally {
      if (!_closed) {
        _busy = false;
        notifyListeners();
      }
    }
  }

  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    super.dispose();
    final files = _files.values.toList();
    _files.clear();
    for (final field in fields.where((field) => field.secret)) {
      _values[field.id] = '';
    }
    await Future.wait(files.map((file) => file.dispose()));
  }
}
