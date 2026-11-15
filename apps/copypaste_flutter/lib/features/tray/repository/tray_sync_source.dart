import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/generated/api/device_sync.dart' as runtime;

import '../../devices/devices_gateway.dart' show DeviceSyncPolicy;
import '../../devices/flutter_rust_devices_gateway.dart';
import '../models/tray_menu_model.dart';

/// The device sync policies the tray menu reads and changes.
abstract interface class TraySyncSource {
  Future<TraySyncState> load();

  /// Sets this device's policy for a null [deviceId], else one peer's.
  Future<void> set(String? deviceId, DeviceSyncPolicy policy);
}

class RuntimeTraySyncSource implements TraySyncSource {
  const RuntimeTraySyncSource();

  @override
  Future<TraySyncState> load() async {
    final preferences = deviceSyncPreferencesOf(
      await runtime.deviceSyncPreferences(),
    );
    return TraySyncState(
      thisDevice: preferences.thisDevice,
      devices: [
        for (final peer in await runtime.listPeers())
          if (peer.deviceId case final deviceId?)
            TraySyncDevice(
              deviceId: deviceId,
              name: peer.name,
              policy: preferences.forDevice(deviceId),
            ),
      ],
    );
  }

  @override
  Future<void> set(String? deviceId, DeviceSyncPolicy policy) async {
    final value = runtimeDeviceSyncPolicy(policy);
    if (deviceId == null) {
      await runtime.setThisDeviceSync(policy: value);
    } else {
      await runtime.setDeviceSyncPolicy(deviceId: deviceId, policy: value);
    }
  }
}
