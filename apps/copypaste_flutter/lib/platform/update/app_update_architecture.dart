import 'dart:ffi';

import '../../features/update/models/app_update_models.dart';

/// Selects an APK for the running process; other Android ABIs use universal.
AndroidAppUpdateArchitecture? currentAndroidUpdateArchitecture({Abi? abi}) =>
    switch (abi ?? Abi.current()) {
      Abi.androidArm64 => AndroidAppUpdateArchitecture.arm64,
      Abi.androidArm => AndroidAppUpdateArchitecture.armv7,
      _ => null,
    };
