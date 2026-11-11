import '../models/history_models.dart';
import '../repository/history_repository.dart';
import 'history_scope.dart';

/// Keeps the exact size of the groups the list displays.
///
/// It asks only when the query, the displayed days or the history changed,
/// with one request in flight and the latest state replayed after it. A count
/// it cannot stand behind is withheld rather than shown.
class HistoryGroupCountsController {
  HistoryGroupCountsController(this._scope, {required bool enabled})
    : _repository = switch (_scope.repository) {
        final HistoryGroupCountsRepository repository when enabled =>
          repository,
        _ => null,
      };

  /// Most days one request names; later days stay uncounted.
  static const maxDays = 256;

  final HistoryScope _scope;
  final HistoryGroupCountsRepository? _repository;
  HistoryGroupCounts? _counts;
  HistoryQuery? _query;
  List<DateTime> _days = const [];
  Set<DateTime> _requested = const {};
  bool _uncountable = false;
  bool _stale = false;
  bool _running = false;
  int _generation = 0;

  bool get enabled => _repository != null;

  /// Null while unknown, and whenever the runtime cannot count the query.
  HistoryGroupCounts? get counts => _counts;

  /// [days] are the local calendar days of the unpinned rows now displayed
  /// and [listed] says whether any row is. [exact] is false when a displayed
  /// page hid rows it could not decrypt: the runtime counts those rows, so
  /// its numbers would overstate the list.
  void sync({
    required HistoryQuery query,
    required List<DateTime> days,
    required bool listed,
    required bool exact,
    bool historyChanged = false,
  }) {
    if (_repository == null || _scope.disposed) return;
    final queryChanged = !identical(query, _query);
    if (queryChanged) {
      _query = query;
      _uncountable = query.sort == HistorySort.relevance;
      _generation++;
      _counts = null;
    }
    if (!listed || !exact || _uncountable) {
      _generation++;
      _stale = false;
      _counts = null;
      _requested = const {};
      return;
    }
    if (days.length > maxDays) days = days.sublist(0, maxDays);
    // Days already asked about under this query and history need no new read.
    if (!queryChanged && !historyChanged && _requested.containsAll(days)) {
      return;
    }
    _days = days;
    _requested = days.toSet();
    _stale = true;
    if (!_running) _run();
  }

  Future<void> _run() async {
    _running = true;
    try {
      while (_stale && !_scope.disposed && !_scope.suspended) {
        _stale = false;
        final query = _query!;
        final days = _days;
        final generation = _generation;
        HistoryGroupCounts? next;
        var answered = false;
        try {
          next = await _repository!.groupCounts(query: query, days: days);
          answered = true;
        } catch (_) {
          // An unknown count is withheld. The failed days stay requested, so
          // only a change of query, days or history asks again.
        }
        if (_scope.disposed || generation != _generation) continue;
        if (answered && next == null) _uncountable = true;
        if (next != _counts) {
          _counts = next;
          _scope.notify();
        }
      }
    } finally {
      _running = false;
    }
  }

  void clear() {
    _generation++;
    _stale = false;
    _counts = null;
    _query = null;
    _days = const [];
    _requested = const {};
    _uncountable = false;
  }
}
