import 'dart:io';

import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../files/runtime_data_directory.dart';
import '../startup/launch_at_login.dart';

/// Deletes everything CopyPaste keeps on this device, then ends the process.
abstract interface class LocalDataEraser {
  Future<void> eraseAndExit();
}

class PlatformLocalDataEraser implements LocalDataEraser {
  const PlatformLocalDataEraser();

  static const _channel = MethodChannel('com.copypaste.app/lifecycle');

  @override
  Future<void> eraseAndExit() async {
    // Android clears the app sandbox and its Keystore keys, then kills it.
    if (Platform.isAndroid) {
      await _channel.invokeMethod<void>('eraseLocalData');
      return;
    }
    await runtime.eraseDesktopRuntimeData(
      daemonExecutable: desktopDaemonExecutable().path,
    );
    // History and its key are gone; what follows removes what remains.
    await _quietly(() => platformLaunchAtLoginGateway().setEnabled(false));
    await _quietly(() async => (await SharedPreferences.getInstance()).clear());
    await _quietly(() async => _delete(await getApplicationCacheDirectory()));
    final temporary = await getTemporaryDirectory();
    for (final name in const ['copypaste-staging', 'copypaste-updates']) {
      await _quietly(() => _delete(Directory('${temporary.path}/$name')));
    }
    // A Windows debug build shares this folder with an installed release.
    if (kReleaseMode) {
      final support = await getApplicationSupportDirectory();
      await _quietly(() => _delete(support));
      if (Platform.isWindows) await _quietly(() => support.parent.delete());
    }
    // macOS also drops its defaults domain, WebKit data and privacy grants.
    if (Platform.isMacOS) {
      await _quietly(() => _channel.invokeMethod<void>('eraseLocalData'));
    }
    exit(0);
  }

  static Future<void> _delete(Directory directory) async {
    if (await directory.exists()) await directory.delete(recursive: true);
  }

  static Future<void> _quietly(Future<Object?> Function() step) async {
    try {
      await step();
    } on Object {
      // Leftovers must not keep a half-erased app running.
    }
  }
}
