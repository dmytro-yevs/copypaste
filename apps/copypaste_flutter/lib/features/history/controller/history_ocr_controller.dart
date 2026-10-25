import 'package:flutter/foundation.dart';

import '../../../platform/clipboard/clipboard_writer.dart';
import '../../modules/controller/modules_controller.dart';
import '../../modules/models/module_models.dart';
import '../models/history_models.dart';
import '../repository/history_image_input.dart';

/// Owns OCR availability, invocation, and result actions outside the inspector.
class HistoryOcrController extends ChangeNotifier {
  HistoryOcrController({
    required ModulesController modules,
    required HistoryImageInput imageInput,
    ClipboardWriter clipboard = const SystemClipboardWriter(),
    Future<void> Function(String id, String text)? copyText,
  }) : _modules = modules,
       _imageInput = imageInput,
       _clipboard = clipboard,
       _copyText = copyText {
    _modules.addListener(_modulesChanged);
  }

  final ModulesController _modules;
  final HistoryImageInput _imageInput;
  final ClipboardWriter _clipboard;
  final Future<void> Function(String id, String text)? _copyText;
  String? _clipId;
  bool _busy = false;
  bool _disposed = false;
  String? _text;
  String? _error;

  bool get available {
    final module = _modules.installedModule('copypaste.ocr');
    return _modules.state == ModulesLoadState.ready &&
        module != null &&
        module.enabled &&
        module.error == null &&
        !module.restartRequired &&
        module.commands.any((command) => command.id == 'recognize-image');
  }

  bool get canRun => !_disposed && available && !_busy && !_modules.busy;
  bool get busy => _busy;
  String? get text => _text;
  String? get errorMessage => _error;

  Future<void> recognize(HistoryClip clip) async {
    if (!canRun || clip.contentKind != HistoryClipKind.image) return;
    _busy = true;
    _text = null;
    _clipId = clip.id;
    _error = null;
    notifyListeners();
    try {
      await _modules.initialize();
      if (_disposed) return;
      if (!available) {
        _error =
            _modules.errorMessage ??
            'OCR is unavailable. Check the module in Settings and try again.';
        return;
      }
      final input = await _imageInput.prepare(clip);
      try {
        if (_disposed) return;
        final result = await _modules.invoke(
          'copypaste.ocr',
          'recognize-image',
          {'image_path': input.path},
        );
        if (_disposed) return;
        if (result == null) {
          _error =
              _modules.errorMessage ?? 'OCR could not be started. Try again.';
        } else {
          _text = result.text;
        }
      } finally {
        await input.dispose();
      }
    } catch (_) {
      if (!_disposed) _error = 'Image text could not be recognized. Try again.';
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> copyText() async {
    final text = _text;
    if (_disposed || _busy || text == null || text.trim().isEmpty) return false;
    return copySelection(text);
  }

  Future<bool> copySelection(String text) async {
    if (_disposed || _busy || text.isEmpty) return false;
    final id = _clipId;
    try {
      if (id != null && _copyText != null) {
        await _copyText(id, text);
      } else {
        await _clipboard.writeText(text);
      }
      return true;
    } catch (_) {
      return false;
    }
  }

  void _modulesChanged() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _modules.removeListener(_modulesChanged);
    super.dispose();
  }
}
