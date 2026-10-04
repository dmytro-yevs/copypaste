import 'dart:convert';
import 'dart:io';

import 'package:copypaste_flutter/features/update/update.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const publicKey = 'RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3';
  const signature = '''
untrusted comment: signature from minisign secret key
RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
trusted comment: timestamp:1556193335\tfile:test
y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==
''';

  test(
    'verifies a prehashed minisign signature and trusted filename',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'copypaste-minisign-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final file = File('${directory.path}/test');
      await file.writeAsString('test');
      final verifier = MinisignVerifier(publicKeyBase64: publicKey);

      expect(
        await verifier.verifyFile(
          file: file,
          encodedSignature: base64.encode(utf8.encode(signature.trim())),
          expectedFileName: 'test',
        ),
        isTrue,
      );
    },
  );

  test('rejects a signature whose trusted filename was changed', () async {
    final directory = await Directory.systemTemp.createTemp(
      'copypaste-minisign-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final file = File('${directory.path}/test');
    await file.writeAsString('test');
    final verifier = MinisignVerifier(publicKeyBase64: publicKey);

    expect(
      await verifier.verifyFile(
        file: file,
        encodedSignature: base64.encode(utf8.encode(signature.trim())),
        expectedFileName: 'other',
      ),
      isFalse,
    );
  });
}
