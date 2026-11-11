import 'package:copypaste_flutter/l10n/l10n.dart';

enum HistoryAction { copy, download, importFiles, pin, delete }

/// The History status message and the latest failed user operation.
class HistoryErrors {
  static String get watchUnavailable =>
      AppStrings.current.historyErrorUpdatesUnavailable;

  String? message;
  String? _operationError;
  HistoryAction? _failedAction;
  int _operationErrorRevision = 0;

  String? get operationError => _operationError;
  HistoryAction? get failedAction => _failedAction;
  int get operationErrorRevision => _operationErrorRevision;

  void reportOperation(String failure, {required HistoryAction action}) {
    message = failure;
    _operationError = failure;
    _failedAction = action;
    _operationErrorRevision++;
  }
}
