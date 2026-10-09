import 'dart:ffi';

import 'package:flutter/services.dart';
import 'package:pub_semver/pub_semver.dart';

import '../../features/modules/models/module_marketplace_models.dart';
import '../../features/modules/models/module_models.dart';
import '../update/app_update_platform.dart';
import '../../features/update/models/app_update_models.dart';

/// Uses the running process ABI, including apps running under emulation.
class ModuleMarketplacePlatform {
  ModuleMarketplacePlatform({
    AppUpdatePlatform? appPlatform,
    MethodChannel? systemChannel,
    Abi Function()? currentAbi,
  }) : _appPlatform = appPlatform ?? MethodChannelAppUpdatePlatform(),
       _systemChannel =
           systemChannel ?? const MethodChannel('com.copypaste.app/app_update'),
       _currentAbi = currentAbi ?? Abi.current;

  final AppUpdatePlatform _appPlatform;
  final MethodChannel _systemChannel;
  final Abi Function() _currentAbi;

  Future<ModuleMarketplaceTarget> currentTarget() async {
    final (platform, architecture) = targetForAbi(_currentAbi());
    return ModuleMarketplaceTarget(
      platform: platform,
      architecture: architecture,
      appVersion: await _readVersion(_appPlatform.currentVersion),
      systemVersion: await _readVersion(
        () => _systemChannel.invokeMethod<String>('systemVersion'),
        system: true,
      ),
    );
  }

  Future<Version?> _readVersion(
    Future<String?> Function() read, {
    bool system = false,
  }) async {
    try {
      final value = await read();
      if (value == null) return null;
      if (!system) return Version.parse(value.trim());
      final match = RegExp(r'^(\d+)(?:\.(\d+))?(?:\.(\d+))?').firstMatch(value);
      if (match == null) return null;
      return Version.parse(
        '${match.group(1)}.${match.group(2) ?? '0'}.${match.group(3) ?? '0'}',
      );
    } on AppUpdateException {
      return null;
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    } on FormatException {
      return null;
    }
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
    Abi.linuxX64 => ('linux', 'x86_64'),
    Abi.linuxArm64 => ('linux', 'aarch64'),
    _ => throw const ModulesException(
      'Modules are unavailable on this device.',
    ),
  };
}
