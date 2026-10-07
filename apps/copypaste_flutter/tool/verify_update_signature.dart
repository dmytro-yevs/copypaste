import 'dart:io';

import 'package:copypaste_flutter/features/update/repository/minisign_verifier.dart';

Future<void> main(List<String> arguments) async {
  if (arguments.length != 2) {
    stderr.writeln(
      'usage: dart --disable-dart-dev --packages=.dart_tool/package_config.json '
      'tool/verify_update_signature.dart <artifact> <signature>',
    );
    exitCode = 2;
    return;
  }
  final artifact = File(arguments[0]);
  final signature = File(arguments[1]);
  if (!await artifact.exists() || !await signature.exists()) {
    stderr.writeln('update artifact or signature is missing');
    exitCode = 1;
    return;
  }
  final valid =
      await MinisignVerifier(
        publicKeyBase64: copyPasteUpdaterPublicKey,
      ).verifyFile(
        file: artifact,
        encodedSignature: await signature.readAsString(),
        expectedFileName: artifact.uri.pathSegments.last,
      );
  if (!valid) {
    stderr.writeln('update signature does not match the production public key');
    exitCode = 1;
  }
}
