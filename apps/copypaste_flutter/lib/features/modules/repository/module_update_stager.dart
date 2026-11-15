import 'package:pub_semver/pub_semver.dart';

import '../../../generated/api.dart' as runtime;

/// Stages authenticated packages in the runtime's persistent module storage.
abstract interface class ModuleUpdateStager {
  Future<void> stage({
    required String id,
    required String packagePath,
    required Version appVersion,
  });
}

class RuntimeModuleUpdateStager implements ModuleUpdateStager {
  @override
  Future<void> stage({
    required String id,
    required String packagePath,
    required Version appVersion,
  }) => runtime.moduleStageUpdate(
    id: id,
    packagePath: packagePath,
    appVersion: appVersion.toString(),
  );
}
