import 'package:copypaste_flutter/features/diagnostics/models/diagnostic_models.dart';
import 'package:copypaste_flutter/features/diagnostics/repository/diagnostics_repository.dart';

/// Newest-first in-memory log; the cursor is the index of the next row.
class FakeDiagnosticsRepository implements DiagnosticsRepository {
  FakeDiagnosticsRepository([List<DiagnosticEntry>? entries])
    : entries = entries ?? [];

  final List<DiagnosticEntry> entries;
  final requests = <({DiagnosticFilter filter, String? cursor, int limit})>[];
  final exported = <String>[];
  final presented = <String>[];
  String? exportPath = 'diagnostics.txt';
  bool failExport = false;

  @override
  Future<DiagnosticPage> page(
    DiagnosticFilter filter, {
    String? cursor,
    int limit = DiagnosticLimits.pageSize,
  }) async {
    requests.add((filter: filter, cursor: cursor, limit: limit));
    final matching = entries
        .where(
          (entry) =>
              (filter.severity == null || entry.severity == filter.severity) &&
              (filter.source == null || entry.source == filter.source) &&
              entry.message.contains(filter.query),
        )
        .toList();
    final start = cursor == null ? 0 : int.parse(cursor);
    final end = (start + limit).clamp(0, matching.length);
    return DiagnosticPage(
      entries: matching.sublist(start, end),
      nextCursor: end < matching.length ? '$end' : null,
    );
  }

  @override
  Future<String?> chooseExportPath() async => exportPath;

  @override
  Future<void> export(String path) async {
    if (failExport) throw StateError('disk full');
    exported.add(path);
  }

  @override
  Future<void> present(String path) async => presented.add(path);
}

DiagnosticEntry diagnosticEntry(
  int n, {
  DiagnosticSeverity severity = DiagnosticSeverity.info,
  DiagnosticSource source = DiagnosticSource.daemon,
}) => DiagnosticEntry(
  at: DateTime.utc(2026, 11, 15, 10, 0, n % 60),
  severity: severity,
  source: source,
  target: 'copypaste::sync',
  message: 'event $n',
);
