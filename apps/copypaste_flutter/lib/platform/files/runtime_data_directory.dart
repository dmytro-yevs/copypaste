import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';

/// The desktop daemon executable bundled beside the application binary.
File desktopDaemonExecutable() => File(
  '${File(Platform.resolvedExecutable).parent.path}/'
  '${Platform.isWindows ? 'copypaste-daemon.exe' : 'copypaste-daemon'}',
);

/// The directory the desktop runtime keeps history, logs and backups in.
Future<Directory> runtimeDataDirectory() async {
  final support = await getApplicationSupportDirectory();
  return Directory(
    '${support.path}/${kDebugMode ? 'development-runtime' : 'runtime'}',
  );
}
