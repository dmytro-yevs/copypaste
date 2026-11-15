import 'dart:async';

import 'package:flutter/foundation.dart';

import '../models/diagnostic_models.dart';
import '../repository/diagnostics_repository.dart';

enum DiagnosticsStatus { idle, loading, ready, failed }

/// Pages the runtime log while a view is attached and keeps only the page
/// shown; detaching releases it. The log only pages forward, so the cursor of
/// every page reached is kept to go back.
class DiagnosticsController extends ChangeNotifier {
  DiagnosticsController({
    required DiagnosticsRepository repository,
    this.searchDelay = const Duration(milliseconds: 300),
  }) : _repository = repository;

  final DiagnosticsRepository _repository;
  final Duration searchDelay;

  DiagnosticsStatus _status = DiagnosticsStatus.idle;
  DiagnosticFilter _filter = const DiagnosticFilter();
  List<DiagnosticEntry> _entries = const [];
  // The cursor that loads each known page; the first page has none.
  List<String?> _pageCursors = const [null];
  int _page = 0;
  bool _loadingPage = false;
  bool _exporting = false;
  int _attached = 0;
  int _generation = 0;
  Timer? _search;
  bool _disposed = false;

  DiagnosticsStatus get status => _status;
  DiagnosticFilter get filter => _filter;
  List<DiagnosticEntry> get entries => _entries;
  bool get loadingPage => _loadingPage;

  /// The page shown, from zero.
  int get page => _page;

  /// Pages that can be opened: those reached and the next one, if any.
  int get pageCount => _pageCursors.length;
  bool get exporting => _exporting;
  bool get busy => _exporting;

  void attach() {
    if (_attached++ > 0) return;
    unawaited(refresh());
  }

  void detach() {
    if (_attached == 0 || --_attached > 0) return;
    _search?.cancel();
    _generation++;
    _entries = const [];
    _pageCursors = const [null];
    _page = 0;
    _loadingPage = false;
    _status = DiagnosticsStatus.idle;
  }

  void setSeverity(DiagnosticSeverity? severity) => _setFilter(
    DiagnosticFilter(
      severity: severity,
      source: _filter.source,
      query: _filter.query,
    ),
  );

  void setSource(DiagnosticSource? source) => _setFilter(
    DiagnosticFilter(
      severity: _filter.severity,
      source: source,
      query: _filter.query,
    ),
  );

  void setQuery(String query) {
    final trimmed = query.trim();
    if (trimmed == _filter.query) return;
    _filter = DiagnosticFilter(
      severity: _filter.severity,
      source: _filter.source,
      query: trimmed,
    );
    _search?.cancel();
    _search = Timer(searchDelay, () => unawaited(refresh()));
  }

  void _setFilter(DiagnosticFilter filter) {
    _search?.cancel();
    _filter = filter;
    unawaited(refresh());
  }

  /// Reloads from the newest event; a cursor never outlives its filter.
  Future<void> refresh() async {
    final generation = ++_generation;
    _loadingPage = false;
    _pageCursors = const [null];
    _page = 0;
    _status = DiagnosticsStatus.loading;
    notifyListeners();
    try {
      final page = await _repository.page(_filter);
      if (_disposed || generation != _generation) return;
      _show(0, page);
      _status = DiagnosticsStatus.ready;
    } on Object {
      if (_disposed || generation != _generation) return;
      _entries = const [];
      _status = DiagnosticsStatus.failed;
    }
    notifyListeners();
  }

  /// Shows page [index]; a failed load keeps the page shown.
  Future<void> openPage(int index) async {
    if (index < 0 || index >= pageCount || index == _page || _loadingPage) {
      return;
    }
    final generation = _generation;
    _loadingPage = true;
    notifyListeners();
    try {
      final page = await _repository.page(
        _filter,
        cursor: _pageCursors[index],
        limit: DiagnosticLimits.pageSize,
      );
      if (_disposed || generation != _generation) return;
      _show(index, page);
    } on Object {
      if (_disposed || generation != _generation) return;
    } finally {
      if (!_disposed && generation == _generation) _loadingPage = false;
    }
    notifyListeners();
  }

  void _show(int index, DiagnosticPage page) {
    _entries = List.unmodifiable(page.entries);
    _page = index;
    if (index == _pageCursors.length - 1 && page.nextCursor != null) {
      _pageCursors = List.unmodifiable([..._pageCursors, page.nextCursor]);
    }
  }

  Future<DiagnosticExportOutcome> export() async {
    if (_exporting) return DiagnosticExportOutcome.cancelled;
    _exporting = true;
    notifyListeners();
    try {
      final path = await _repository.chooseExportPath();
      if (path == null) return DiagnosticExportOutcome.cancelled;
      await _repository.export(path);
      await _repository.present(path);
      return DiagnosticExportOutcome.saved;
    } on Object {
      return DiagnosticExportOutcome.failed;
    } finally {
      _exporting = false;
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
    _search?.cancel();
    super.dispose();
  }
}
