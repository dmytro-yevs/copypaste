import 'package:flutter/foundation.dart';
import '../models/module_models.dart';
import '../repository/modules_repository.dart';

/// Owns transient form values and selected input-file leases until invocation ends.
class ModuleFormDraft extends ChangeNotifier {
  ModuleFormDraft({
    required this.fields,
    required Map<String, Object> initial,
    ModuleInputPicker? picker,
    this.models = const [],
    this.languageField,
  }) : _picker = picker,
       _values = {
         for (final field in fields)
           field.id: initial[field.id] ?? field.defaultValue,
       };
  final List<ModuleField> fields;
  final List<ModuleSearchModel> models;
  final String? languageField;
  ModuleSearchModel? get selectedModel {
    final value = _values[languageField];
    return value is List
        ? ModuleSearchModel.forLanguages(models, value.cast<String>())
        : null;
  }

  final ModuleInputPicker? _picker;
  final Map<String, Object> _values;
  final Map<String, SelectedModuleInput> _files = {};
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
        if (field.kind == ModuleFieldKind.choices) {
          if (value is! List || (field.required && value.isEmpty)) return false;
          return value.length == value.toSet().length &&
              value.every(
                (id) => field.options.any((option) => option.id == id),
              );
        }
        return !field.required ||
            field.kind == ModuleFieldKind.boolean ||
            (value is String && value.trim().isNotEmpty);
      }) &&
      (models.isEmpty || selectedModel != null);

  void setValue(String id, Object value) {
    if (_closed) return;
    _values[id] = value;
    notifyListeners();
  }

  Future<void> chooseFile(ModuleField field) async {
    if (_busy || _closed) return;
    _busy = true;
    _error = null;
    notifyListeners();
    try {
      final picker = _picker;
      if (picker == null) {
        throw const ModulesException('File selection is unavailable.');
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
        _error = error is ModulesException
            ? error.message
            : 'The file could not be selected.';
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
