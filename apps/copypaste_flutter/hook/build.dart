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
  return values;
}
