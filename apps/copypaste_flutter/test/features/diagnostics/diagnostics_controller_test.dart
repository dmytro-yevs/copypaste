import 'package:copypaste_flutter/features/diagnostics/controller/diagnostics_controller.dart';
import 'package:copypaste_flutter/features/diagnostics/models/diagnostic_models.dart';
import 'package:flutter_test/flutter_test.dart';

import 'diagnostics_test_support.dart';

void main() {
  test('keeps one page, goes back, and releases it on detach', () async {
    final repository = FakeDiagnosticsRepository([
      for (var n = 0; n < 120; n++) diagnosticEntry(n),
    ]);
    final controller = DiagnosticsController(repository: repository);
    addTearDown(controller.dispose);

    controller.attach();
    await pumpEventQueue();
    expect(controller.entries, hasLength(DiagnosticLimits.pageSize));
    expect(controller.pageCount, 2);
    await controller.openPage(1);
    await controller.openPage(2);

    expect(controller.page, 2);
    expect(controller.pageCount, 3);
    expect(controller.entries.first.message, 'event 100');
    expect(controller.entries, hasLength(20));

    await controller.openPage(0);
    expect(controller.entries.first.message, 'event 0');
    expect(controller.entries, hasLength(DiagnosticLimits.pageSize));
    expect(
      repository.requests.every(
        (request) => request.limit <= DiagnosticLimits.pageSize,
      ),
      isTrue,
    );

    controller.detach();
    expect(controller.entries, isEmpty);
    expect(controller.status, DiagnosticsStatus.idle);
  });

  test('a filter change restarts from the newest event', () async {
    final repository = FakeDiagnosticsRepository([
      for (var n = 0; n < 120; n++)
        diagnosticEntry(
          n,
          severity: n.isEven
              ? DiagnosticSeverity.error
              : DiagnosticSeverity.info,
        ),
    ]);
    final controller = DiagnosticsController(
      repository: repository,
      searchDelay: Duration.zero,
    );
    addTearDown(controller.dispose);
    controller.attach();
    await pumpEventQueue();
    await controller.openPage(1);

    controller.setSeverity(DiagnosticSeverity.error);
    await pumpEventQueue();

    expect(repository.requests.last.cursor, isNull);
    expect(controller.page, 0);
    expect(controller.entries.first.message, 'event 0');
    expect(
      controller.entries.every(
        (entry) => entry.severity == DiagnosticSeverity.error,
      ),
      isTrue,
    );

    controller.setQuery(' event 1 ');
    await pumpEventQueue();
    expect(repository.requests.last.filter.query, 'event 1');
    expect(repository.requests.last.filter.severity, DiagnosticSeverity.error);
  });

  test('export saves and presents, and reports a cancelled save', () async {
    final repository = FakeDiagnosticsRepository();
    final controller = DiagnosticsController(repository: repository);
    addTearDown(controller.dispose);

    expect(await controller.export(), DiagnosticExportOutcome.saved);
    expect(repository.exported, ['diagnostics.txt']);
    expect(repository.presented, ['diagnostics.txt']);

    repository.exportPath = null;
    expect(await controller.export(), DiagnosticExportOutcome.cancelled);

    repository
      ..exportPath = 'again.txt'
      ..failExport = true;
    expect(await controller.export(), DiagnosticExportOutcome.failed);
    expect(repository.presented, ['diagnostics.txt']);
    expect(controller.exporting, isFalse);
  });
}
