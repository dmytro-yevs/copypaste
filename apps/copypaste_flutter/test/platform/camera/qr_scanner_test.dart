import 'dart:async';
import 'dart:typed_data';

import 'package:copypaste_flutter/platform/camera/qr_frame.dart';
import 'package:copypaste_flutter/platform/camera/qr_scanner.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:zxing2/qrcode.dart';

void main() {
  test(
    'allows one decode in flight and submits no stale payload after close',
    () async {
      final decoder = _CompletingDecoder();
      final sink = _FakeSink();
      final progress = <QrScanStatus>[];
      final coordinator = QrScanCoordinator(
        decoder: decoder,
        payloadSink: sink,
        onProgress: (event) => progress.add(event.status),
        minimumFrameInterval: Duration.zero,
      );

      final first = coordinator.submit(_frame());
      await coordinator.submit(_frame());
      expect(decoder.calls, 1);

      coordinator.close();
      decoder.complete('pairing-payload');
      await first;

      expect(sink.payloads, isEmpty);
      expect(progress, <QrScanStatus>[QrScanStatus.scanning]);
    },
  );

  test(
    'reports accepted only after the backend accepts a decoded payload',
    () async {
      final progress = <QrScanStatus>[];
      final sink = _FakeSink();
      final coordinator = QrScanCoordinator(
        decoder: _ImmediateDecoder('pairing-payload'),
        payloadSink: sink,
        onProgress: (event) => progress.add(event.status),
        minimumFrameInterval: Duration.zero,
      );

      await coordinator.submit(_frame());

      expect(sink.payloads, <String>['pairing-payload']);
      expect(progress, <QrScanStatus>[
        QrScanStatus.scanning,
        QrScanStatus.accepted,
      ]);
    },
  );

  test('drops a duplicate frame while the cadence limit is active', () async {
    final decoder = _ImmediateDecoder(null);
    var now = DateTime(2026);
    final coordinator = QrScanCoordinator(
      decoder: decoder,
      payloadSink: _FakeSink(),
      onProgress: (_) {},
      minimumFrameInterval: const Duration(seconds: 1),
      clock: () => now,
    );

    await coordinator.submit(_frame());
    await coordinator.submit(_frame());
    now = now.add(const Duration(seconds: 1));
    await coordinator.submit(_frame());

    expect(decoder.calls, 2);
  });

  test('decodes a QR luminance frame', () async {
    final qrCode = Encoder.encode('PAIRING', ErrorCorrectionLevel.m);
    final matrix = qrCode.matrix!;
    const quietZone = 4;
    const moduleSize = 4;
    final width = (matrix.width + quietZone * 2) * moduleSize;
    final bytes = Int8List(width * width);
    for (var y = 0; y < width; y++) {
      for (var x = 0; x < width; x++) {
        final matrixX = x ~/ moduleSize - quietZone;
        final matrixY = y ~/ moduleSize - quietZone;
        final isDark =
            matrixX >= 0 &&
            matrixX < matrix.width &&
            matrixY >= 0 &&
            matrixY < matrix.height &&
            matrix.get(matrixX, matrixY) == 1;
        bytes[y * width + x] = isDark ? 0 : -1;
      }
    }

    final result = await const ZxingQrFrameDecoder().decode(
      QrLuminanceFrame(width: width, height: width, bytes: bytes),
    );

    expect(result, 'PAIRING');
  });
}

QrCameraFrame _frame() => QrCameraFrame(
  width: 2,
  height: 2,
  format: QrFrameFormat.yuv420,
  planes: <QrFramePlane>[
    QrFramePlane(
      bytes: Uint8List.fromList(<int>[0, 1, 2, 3]),
      bytesPerRow: 2,
      bytesPerPixel: 1,
    ),
  ],
);

class _ImmediateDecoder implements QrFrameDecoder {
  _ImmediateDecoder(this.value);

  final String? value;
  int calls = 0;

  @override
  Future<String?> decode(QrLuminanceFrame frame) async {
    calls++;
    return value;
  }
}

class _CompletingDecoder implements QrFrameDecoder {
  final Completer<String?> _completer = Completer<String?>();
  int calls = 0;

  @override
  Future<String?> decode(QrLuminanceFrame frame) {
    calls++;
    return _completer.future;
  }

  void complete(String payload) => _completer.complete(payload);
}

class _FakeSink implements PairingQrPayloadSink {
  final List<String> payloads = <String>[];

  @override
  Future<QrPayloadDisposition> submit(String payload) async {
    payloads.add(payload);
    return QrPayloadDisposition.accepted;
  }
}
