import 'dart:async';
import 'dart:typed_data';

import 'package:zxing2/qrcode.dart';

import 'qr_frame.dart';

/// Owns validation of decoded pairing payloads. Implementations are backend
/// adapters; widgets receive only [QrScanProgress], never the payload.
abstract interface class PairingQrPayloadSink {
  Future<QrPayloadDisposition> submit(String payload);
}

enum QrPayloadDisposition { accepted, rejected, expired, cancelled }

enum QrScanStatus { idle, scanning, accepted, rejected, unavailable }

class QrScanProgress {
  const QrScanProgress(this.status);

  final QrScanStatus status;
}

abstract interface class QrFrameDecoder {
  Future<String?> decode(QrLuminanceFrame frame);
}

/// A QR-only ZXing decoder. Payload parsing and pairing authority remain in the
/// backend sink, so scanner output never becomes a UI data model.
class ZxingQrFrameDecoder implements QrFrameDecoder {
  const ZxingQrFrameDecoder();

  @override
  Future<String?> decode(QrLuminanceFrame frame) async {
    try {
      final pixels = Int32List(frame.bytes.length);
      for (var index = 0; index < frame.bytes.length; index++) {
        final luminance = frame.bytes[index] & 0xff;
        pixels[index] =
            0xff000000 | (luminance << 16) | (luminance << 8) | luminance;
      }
      final source = RGBLuminanceSource(frame.width, frame.height, pixels);
      final bitmap = BinaryBitmap(HybridBinarizer(source));
      return QRCodeReader().decode(bitmap).text;
    } on ReaderException {
      return null;
    } on ArgumentError {
      return null;
    }
  }
}

/// Limits scanner work to one in-flight decode and drops frames that arrive
/// after disposal or after their camera session has been replaced.
class QrScanCoordinator {
  QrScanCoordinator({
    required QrFrameDecoder decoder,
    required PairingQrPayloadSink payloadSink,
    required void Function(QrScanProgress progress) onProgress,
    QrFrameNormalizer normalizer = const QrFrameNormalizer(),
    this.minimumFrameInterval = const Duration(milliseconds: 180),
    DateTime Function()? clock,
  }) : _decoder = decoder,
       _payloadSink = payloadSink,
       _onProgress = onProgress,
       _normalizer = normalizer,
       _clock = clock ?? DateTime.now;

  final QrFrameDecoder _decoder;
  final PairingQrPayloadSink _payloadSink;
  final void Function(QrScanProgress progress) _onProgress;
  final QrFrameNormalizer _normalizer;
  final Duration minimumFrameInterval;
  final DateTime Function() _clock;

  bool _decoding = false;
  bool _closed = false;
  int _sessionToken = 0;
  DateTime? _lastDecodeStartedAt;

  Future<void> submit(QrCameraFrame frame) async {
    if (_closed || _decoding || !_canDecodeNow()) {
      return;
    }
    _decoding = true;
    _lastDecodeStartedAt = _clock();
    final token = _sessionToken;
    _emit(QrScanStatus.scanning, token);
    try {
      final payload = await _decoder.decode(_normalizer.normalize(frame));
      if (_isStale(token) || payload == null || payload.isEmpty) {
        return;
      }
      final disposition = await _payloadSink.submit(payload);
      if (_isStale(token)) {
        return;
      }
      _emit(
        disposition == QrPayloadDisposition.accepted
            ? QrScanStatus.accepted
            : QrScanStatus.rejected,
        token,
      );
    } on Object {
      if (!_isStale(token)) {
        _emit(QrScanStatus.rejected, token);
      }
    } finally {
      if (!_isStale(token)) {
        _decoding = false;
      }
    }
  }

  void close() {
    _closed = true;
    _sessionToken++;
    _decoding = false;
  }

  bool _canDecodeNow() {
    final previous = _lastDecodeStartedAt;
    return previous == null ||
        _clock().difference(previous) >= minimumFrameInterval;
  }

  bool _isStale(int token) => _closed || token != _sessionToken;

  void _emit(QrScanStatus status, int token) {
    if (!_isStale(token)) {
      _onProgress(QrScanProgress(status));
    }
  }
}
