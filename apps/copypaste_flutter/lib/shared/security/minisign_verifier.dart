import 'dart:convert';
import 'dart:io';

import 'package:cryptography/cryptography.dart';

const copyPasteUpdaterPublicKey =
    'RWRBtdsC8GYRSRvWZGp3o4dZ5DcpN+pkhmcUPoMdUWH25yQiGlbbj4WD';

class MinisignVerifier {
  MinisignVerifier({required this.publicKeyBase64});

  final String publicKeyBase64;

  Future<bool> verifyFile({
    required File file,
    required String encodedSignature,
    required String expectedFileName,
  }) async {
    try {
      final publicKeyPacket = base64.decode(publicKeyBase64);
      if (publicKeyPacket.length != 42 ||
          publicKeyPacket[0] != 0x45 ||
          (publicKeyPacket[1] != 0x64 && publicKeyPacket[1] != 0x44)) {
        return false;
      }
      final signatureText = utf8.decode(base64.decode(encodedSignature.trim()));
      final lines = const LineSplitter().convert(signatureText.trim());
      if (lines.length != 4 ||
          !lines[0].startsWith('untrusted comment: ') ||
          !lines[2].startsWith('trusted comment: ')) {
        return false;
      }
      final signaturePacket = base64.decode(lines[1]);
      final globalSignature = base64.decode(lines[3]);
      if (signaturePacket.length != 74 ||
          globalSignature.length != 64 ||
          signaturePacket[0] != 0x45 ||
          signaturePacket[1] != 0x44 ||
          !_bytesEqual(
            publicKeyPacket.sublist(2, 10),
            signaturePacket.sublist(2, 10),
          )) {
        return false;
      }
      final trustedComment = lines[2].substring('trusted comment: '.length);
      if (!trustedComment.split('\t').contains('file:$expectedFileName')) {
        return false;
      }

      final hashSink = Blake2b(hashLengthInBytes: 64).newHashSink();
      await for (final chunk in file.openRead()) {
        hashSink.add(chunk);
      }
      hashSink.close();
      final hash = await hashSink.hash();
      final publicKey = SimplePublicKey(
        publicKeyPacket.sublist(10, 42),
        type: KeyPairType.ed25519,
      );
      final algorithm = Ed25519();
      final signature = Signature(
        signaturePacket.sublist(10, 74),
        publicKey: publicKey,
      );
      if (!await algorithm.verify(hash.bytes, signature: signature)) {
        return false;
      }
      final globalMessage = <int>[
        ...signature.bytes,
        ...utf8.encode(trustedComment),
      ];
      return await algorithm.verify(
        globalMessage,
        signature: Signature(globalSignature, publicKey: publicKey),
      );
    } on FormatException {
      return false;
    } on ArgumentError {
      return false;
    } on StateError {
      return false;
    }
  }
}

bool _bytesEqual(List<int> first, List<int> second) {
  if (first.length != second.length) return false;
  var difference = 0;
  for (var index = 0; index < first.length; index++) {
    difference |= first[index] ^ second[index];
  }
  return difference == 0;
}
