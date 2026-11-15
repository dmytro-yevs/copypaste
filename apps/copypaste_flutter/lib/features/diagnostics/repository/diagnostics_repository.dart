import 'dart:io';

import 'package:copypaste_flutter/generated/api/diagnostics.dart' as api;
import 'package:file_selector/file_selector.dart';

import '../../../l10n/l10n.dart';
import '../../../platform/files/platform_file_gateway.dart';
import '../models/diagnostic_models.dart';

/// Runtime log access and the native destination for its export.
abstract interface class DiagnosticsRepository {
  Future<DiagnosticPage> page(
    DiagnosticFilter filter, {
    String? cursor,
    int limit = DiagnosticLimits.pageSize,
  });

  /// Where the export goes; null when the user cancels.
  Future<String?> chooseExportPath();

  Future<void> export(String path);

  /// Hands a finished export to the platform, which shares it on Android.
  Future<void> present(String path);
}

class RuntimeDiagnosticsRepository implements DiagnosticsRepository {
  const RuntimeDiagnosticsRepository({
    this.fileGateway = const PlatformFileGateway(),
  });

  final PlatformFileGateway fileGateway;

  static const _fileName = 'copypaste-diagnostics.txt';

  @override
  Future<DiagnosticPage> page(
    DiagnosticFilter filter, {
    String? cursor,
    int limit = DiagnosticLimits.pageSize,
  }) async {
    final page = await api.diagnosticEvents(
      cursor: cursor,
      level: switch (filter.severity) {
        null => null,
        DiagnosticSeverity.error => api.DiagnosticLevel.error,
        DiagnosticSeverity.warn => api.DiagnosticLevel.warn,
        DiagnosticSeverity.info => api.DiagnosticLevel.info,
        DiagnosticSeverity.debug => api.DiagnosticLevel.debug,
        DiagnosticSeverity.trace => api.DiagnosticLevel.trace,
      },
      process: switch (filter.source) {
        null => null,
        DiagnosticSource.app => api.DiagnosticProcess.app,
        DiagnosticSource.daemon => api.DiagnosticProcess.daemon,
      },
      query: filter.query.isEmpty ? null : filter.query,
      limit: limit,
    );
    return DiagnosticPage(
      entries: [
        for (final event in page.events)
          DiagnosticEntry(
            at: DateTime.fromMillisecondsSinceEpoch(event.timestampMs.toInt()),
            severity: switch (event.level) {
              api.DiagnosticLevel.error => DiagnosticSeverity.error,
              api.DiagnosticLevel.warn => DiagnosticSeverity.warn,
              api.DiagnosticLevel.info => DiagnosticSeverity.info,
              api.DiagnosticLevel.debug => DiagnosticSeverity.debug,
              api.DiagnosticLevel.trace => DiagnosticSeverity.trace,
            },
            source: switch (event.process) {
              api.DiagnosticProcess.app => DiagnosticSource.app,
              api.DiagnosticProcess.daemon => DiagnosticSource.daemon,
            },
            target: event.target,
            message: event.message,
          ),
      ],
      nextCursor: page.nextCursor,
    );
  }

  @override
  Future<String?> chooseExportPath() async {
    if (Platform.isAndroid) return fileGateway.androidStagingPath(_fileName);
    return (await getSaveLocation(
      suggestedName: _fileName,
      acceptedTypeGroups: [
        XTypeGroup(
          label: AppStrings.current.diagnosticsExportFileType,
          extensions: const ['txt'],
        ),
      ],
    ))?.path;
  }

  @override
  Future<void> export(String path) => api.exportDiagnostics(path: path);

  @override
  Future<void> present(String path) =>
      fileGateway.presentCreatedFile(path, mimeType: 'text/plain');
}
