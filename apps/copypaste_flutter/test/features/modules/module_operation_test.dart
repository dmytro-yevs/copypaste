import 'package:copypaste_flutter/features/modules/models/module_operation.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const started = ModuleOperation.started(
    moduleId: 'copypaste.text-tools',
    kind: ModuleOperationKind.install,
    totalBytes: 2000,
  );

  test('download reports whole-percent progress and received bytes', () {
    expect(started.phase, ModuleOperationPhase.download);
    expect(started.progress, 0);
    expect(started.downloadedBytes, 0);
    final partial = started.downloadedTo(0.426);
    expect(partial.progress, 0.426);
    expect(partial.downloadPercent, 42);
    expect(partial.downloadedBytes, 852);
    expect(identical(partial.downloadedTo(0.429), partial), isTrue);
    expect(partial.downloadedTo(7).downloadPercent, 100);
  });

  test('installation has no measurable progress', () {
    final installing = started.downloadedTo(1).installing();
    expect(installing.phase, ModuleOperationPhase.install);
    expect(installing.running, isTrue);
    expect(installing.progress, isNull);
    expect(installing.downloadedBytes, 2000);
    expect(identical(installing.downloadedTo(0.5), installing), isTrue);
  });

  test('a failure keeps its phase and stops reporting progress', () {
    final download = started.downloadedTo(0.5).failedInPhase();
    expect(download.failed, isTrue);
    expect(download.phase, ModuleOperationPhase.download);
    expect(download.progress, isNull);
    expect(identical(download.installing(), download), isTrue);
    final install = started.installing().failedInPhase();
    expect(install.failed, isTrue);
    expect(install.phase, ModuleOperationPhase.install);
  });
}
