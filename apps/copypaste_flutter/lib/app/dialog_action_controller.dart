import 'package:flutter/foundation.dart';

/// Owns the pending and failure states shared by modal actions.
class DialogActionController extends ChangeNotifier {
  DialogActionController({
    required Future<bool?> Function() action,
    required String Function() failureMessage,
  }) : _action = action,
       _failureMessage = failureMessage;

  final Future<bool?> Function() _action;
  final String Function() _failureMessage;
  bool _submitting = false;
  bool _disposed = false;
  String? _errorMessage;

  bool get submitting => _submitting;
  String? get errorMessage => _errorMessage;

  /// True completes the dialog, false shows a failure, and null cancels only
  /// the nested interaction (for example, a native file picker).
  Future<bool> submit() async {
    if (_submitting || _disposed) return false;
    _submitting = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final result = await _action();
      if (result == false) _errorMessage = _failureMessage();
      return result == true;
    } catch (_) {
      _errorMessage = _failureMessage();
      return false;
    } finally {
      _submitting = false;
      if (!_disposed) notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
