import 'package:flutter/foundation.dart';
import '../models/module_models.dart';
import '../repository/modules_repository.dart';

/// Owns transient form values and selected input-file leases until invocation ends.
class ModuleFormDraft extends ChangeNotifier {
  ModuleFormDraft({
    required this.fields,
    required Map<String, Object> initial,
    ModuleInputPicker? picker,
  }) : _picker = picker,
       _values = {
         for (final field in fields)
           field.id: initial[field.id] ?? field.defaultValue,
       };
  final List<ModuleField> fields;
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
  bool get valid => fields.every(
    (field) =>
        !field.required ||
        field.kind == ModuleFieldKind.boolean ||
        (_values[field.id] as String).trim().isNotEmpty,
  );

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
    await Future.wait(files.map((file) => file.dispose()));
  }
}
