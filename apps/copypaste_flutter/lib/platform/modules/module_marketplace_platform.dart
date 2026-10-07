import 'dart:ffi';

import 'package:pub_semver/pub_semver.dart';

import '../../features/modules/models/module_marketplace_models.dart';
import '../../features/modules/models/module_models.dart';
import '../update/app_update_platform.dart';

/// Uses the running process ABI, including apps running under emulation.
class ModuleMarketplacePlatform {
  ModuleMarketplacePlatform({AppUpdatePlatform? appPlatform})
    : _appPlatform = appPlatform ?? MethodChannelAppUpdatePlatform();

  final AppUpdatePlatform _appPlatform;

  Future<ModuleMarketplaceTarget> currentTarget() async {
    final (platform, architecture) = targetForAbi(Abi.current());
    return ModuleMarketplaceTarget(
      platform: platform,
      architecture: architecture,
      appVersion: Version.parse(await _appPlatform.currentVersion()),
    );
  }

  static (String, String) targetForAbi(Abi abi) => switch (abi) {
    Abi.macosArm64 => ('macos', 'aarch64'),
    Abi.macosX64 => ('macos', 'x86_64'),
    Abi.windowsArm64 => ('windows', 'aarch64'),
    Abi.windowsX64 => ('windows', 'x86_64'),
    Abi.windowsIA32 => ('windows', 'x86'),
    Abi.androidArm64 => ('android', 'aarch64'),
    Abi.androidArm => ('android', 'arm'),
    Abi.androidX64 => ('android', 'x86_64'),
    Abi.androidIA32 => ('android', 'x86'),
    _ => throw const ModulesException(
      'Modules are unavailable on this device.',
    ),
  };
}
