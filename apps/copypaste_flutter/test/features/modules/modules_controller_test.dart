import 'dart:async';
import 'package:flutter_test/flutter_test.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'modules_test_support.dart';

void main() {
  test(
    'install, disable, preferences, invoke, and remove share repository state',
    () async {
      final repository = MemoryModulesRepository();
      final picker = MemoryModulePicker();
      final controller = ModulesController(
        repository: repository,
        picker: picker,
      );
      await controller.initialize();
      expect(controller.state, ModulesLoadState.ready);
      await controller.install();
      expect(controller.modules.single.id, testModule.id);
      expect(picker.disposedPackages, 1);
      await controller.setEnabled(testModule.id, false);
      expect(controller.modules.single.enabled, false);
      await controller.setPreferences(testModule.id, {'uppercase': true});
      expect(repository.lastPreferences, {'uppercase': true});
      final result = await controller.invoke(testModule.id, 'transform', {
        'text': 'Україна',
      });
      expect(result!.text, 'УКРАЇНА');
      await controller.remove(testModule.id);
      expect(controller.modules, isEmpty);
      controller.dispose();
    },
  );

  test(
    'cancellation has no install side effect and failed installation releases staging',
    () async {
      final repository = MemoryModulesRepository();
      final picker = MemoryModulePicker()..cancel = true;
      final controller = ModulesController(
        repository: repository,
        picker: picker,
      );
      await controller.initialize();
      await controller.install();
      expect(repository.calls, isEmpty);
      picker.cancel = false;
      repository.failure = const ModulesException('Invalid signature.');
      await controller.install();
      expect(controller.errorMessage, 'Invalid signature.');
      expect(picker.disposedPackages, 1);
      expect(controller.busy, false);
      expect(controller.modules, isEmpty);
      controller.dispose();
    },
  );

  test(
    'disposal while installation is pending does not notify disposed state',
    () async {
      final repository = _PendingRepository();
      final picker = MemoryModulePicker();
      final controller = ModulesController(
        repository: repository,
        picker: picker,
      );
      await controller.initialize();
      final operation = controller.install();
      await Future<void>.delayed(Duration.zero);
      controller.dispose();
      repository.pending.complete();
      await operation;
      expect(picker.disposedPackages, 1);
    },
  );
}

class _PendingRepository extends MemoryModulesRepository {
  final pending = Completer<void>();
  @override
  Future<void> install(String packagePath) => pending.future;
}
