enum HistoryAction { copy, download, importFiles, pin, delete }

/// The History status message and the latest failed user operation.
class HistoryErrors {
  static const watchUnavailable =
      'History updates are temporarily unavailable.';

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
