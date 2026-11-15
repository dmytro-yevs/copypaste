/// Severity of one runtime event, most severe first.
enum DiagnosticSeverity { error, warn, info, debug, trace }

/// The process that wrote a runtime event.
enum DiagnosticSource { app, daemon }

/// One redacted runtime event: a literal message, never clipboard content.
class DiagnosticEntry {
  const DiagnosticEntry({
    required this.at,
    required this.severity,
    required this.source,
    required this.target,
    required this.message,
  });

  final DateTime at;
  final DiagnosticSeverity severity;
  final DiagnosticSource source;
  final String target;
  final String message;
}

/// A page of events, newest first; [nextCursor] is null at the end of the log.
class DiagnosticPage {
  const DiagnosticPage({required this.entries, this.nextCursor});

  final List<DiagnosticEntry> entries;
  final String? nextCursor;
}

/// What the viewer lists; `null` fields do not filter.
class DiagnosticFilter {
  const DiagnosticFilter({this.severity, this.source, this.query = ''});

  final DiagnosticSeverity? severity;
  final DiagnosticSource? source;
  final String query;
}

enum DiagnosticExportOutcome { saved, cancelled, failed }

abstract final class DiagnosticLimits {
  /// Rows one page shows, the only rows the viewer keeps; the runtime caps a
  /// page at 100.
  static const pageSize = 50;
}
