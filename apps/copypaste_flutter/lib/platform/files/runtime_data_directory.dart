import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';

/// The directory the desktop runtime keeps history, logs and backups in.
Future<Directory> runtimeDataDirectory() async {
  final support = await getApplicationSupportDirectory();
  return Directory(
    '${support.path}/${kDebugMode ? 'development-runtime' : 'runtime'}',
  );
}
