import 'package:flutter/foundation.dart';

import '../repository/history_repository.dart';

/// The repository, change notification and lifecycle shared by the owners
/// composed into one History controller.
class HistoryScope {
  HistoryScope(this.repository, this.notify);

  final HistoryRepository repository;
  final VoidCallback notify;
  bool disposed = false;
  bool suspended = false;

  /// Advances on every suspension so late responses cannot refill state.
  int generation = 0;
}
