import 'dart:io';

import 'package:flutter_rust_bridge_hooks/flutter_rust_bridge_hooks.dart';

void main(List<String> args) async {
  await build(args, (input, output) async {
    await FlutterRustBridgeNativeAssetsBuilder(
      cratePath: '../../crates/copypaste-flutter-bridge',
      extraCargoEnvironmentVariables: _cargoEnvironment(),
    ).run(input: input, output: output);
  });
}

Map<String, String> _cargoEnvironment() {
  const names = [
    'OPENSSL_NO_VENDOR',
    'OPENSSL_STATIC',
    'OPENSSL_INCLUDE_DIR',
    'OPENSSL_LIB_DIR',
    'OPENSSL_LIBS',
  ];
  final values = <String, String>{};
  for (final name in names) {
    final value = Platform.environment[name];
    if (value != null) values[name] = value;
  }
  if (Platform.isWindows && !values.containsKey('OPENSSL_NO_VENDOR')) {
    final programFiles =
        Platform.environment['ProgramFiles'] ?? r'C:\Program Files';
    final includeDir = '$programFiles\\OpenSSL\\include';
    final libDir = '$programFiles\\OpenSSL\\lib\\VC\\x64\\MT';
    if (File('$includeDir\\openssl\\ssl.h').existsSync() &&
        File('$libDir\\libssl_static.lib').existsSync() &&
        File('$libDir\\libcrypto_static.lib').existsSync()) {
      values.addAll({
        'OPENSSL_NO_VENDOR': '1',
        'OPENSSL_STATIC': '1',
        'OPENSSL_INCLUDE_DIR': includeDir,
        'OPENSSL_LIB_DIR': libDir,
        'OPENSSL_LIBS': 'libssl_static:libcrypto_static',
      });
    }
  }
  return values;
}
