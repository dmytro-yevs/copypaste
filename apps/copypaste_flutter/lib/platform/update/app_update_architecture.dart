import 'dart:ffi';

import '../../features/update/models/app_update_models.dart';

/// Selects an APK for the running process; other Android ABIs use universal.
AndroidAppUpdateArchitecture? currentAndroidUpdateArchitecture({Abi? abi}) =>
    switch (abi ?? Abi.current()) {
      Abi.androidArm64 => AndroidAppUpdateArchitecture.arm64,
      Abi.androidArm => AndroidAppUpdateArchitecture.armv7,
      _ => null,
    };

/// Selects Linux release assets for the architecture of the running process.
LinuxAppUpdateArchitecture? currentLinuxUpdateArchitecture({Abi? abi}) =>
    switch (abi ?? Abi.current()) {
      Abi.linuxX64 => LinuxAppUpdateArchitecture.x86_64,
      Abi.linuxArm64 => LinuxAppUpdateArchitecture.aarch64,
      _ => null,
    };

/// Selects the macOS disk image built for the architecture of the running app.
MacosAppUpdateArchitecture? currentMacosUpdateArchitecture({Abi? abi}) =>
    switch (abi ?? Abi.current()) {
      Abi.macosArm64 => MacosAppUpdateArchitecture.arm64,
      Abi.macosX64 => MacosAppUpdateArchitecture.x86_64,
      _ => null,
    };
