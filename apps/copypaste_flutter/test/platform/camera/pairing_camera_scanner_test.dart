import 'dart:async';
import 'dart:typed_data';

import 'package:copypaste_flutter/platform/camera/pairing_camera_scanner.dart';
import 'package:copypaste_flutter/platform/camera/qr_frame.dart';
import 'package:copypaste_flutter/platform/camera/qr_scanner.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('reports no camera as unavailable', (tester) async {
    final scanner = PairingCameraScanner(
      source: _FakeSource(),
      coordinatorFactory: _coordinator,
    );

    await scanner.start();

    expect(scanner.state.value, PairingCameraState.unavailable);
    await scanner.dispose();
  });

  testWidgets('disposes a late camera lease after scanner disposal', (
    tester,
  ) async {
    final source = _CompletingSource();
    final scanner = PairingCameraScanner(
      source: source,
      coordinatorFactory: _coordinator,
    );

    final starting = scanner.start();
    await scanner.dispose();
    source.complete();
    await starting;

    expect(source.lease.disposed, isTrue);
  });

  testWidgets('stops camera frames on inactivity and ignores a late frame', (
    tester,
  ) async {
    final source = _FakeSource.withLease();
    final sink = _FakeSink();
    final scanner = PairingCameraScanner(
      source: source,
      coordinatorFactory: (progress) => QrScanCoordinator(
        decoder: _ImmediateDecoder(),
        payloadSink: sink,
        onProgress: progress,
        minimumFrameInterval: Duration.zero,
      ),
    );

    await scanner.start();
    scanner.didChangeAppLifecycleState(AppLifecycleState.inactive);
    await tester.pump();
    source.lease.emit(_frame());
    await tester.pump();

    expect(source.lease.stopped, isTrue);
    expect(source.lease.disposed, isTrue);
    expect(sink.payloads, isEmpty);
    await scanner.dispose();
  });
}

QrScanCoordinator _coordinator(void Function(QrScanProgress) progress) =>
    QrScanCoordinator(
      decoder: _ImmediateDecoder(),
      payloadSink: _FakeSink(),
      onProgress: progress,
      minimumFrameInterval: Duration.zero,
    );

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

class _FakeSource implements PairingCameraSource {
  _FakeSource() : lease = _FakeLease(), returnsNull = true;
  _FakeSource.withLease() : lease = _FakeLease(), returnsNull = false;

  final _FakeLease lease;
  final bool returnsNull;

  @override
  Future<PairingCameraLease?> open() async => returnsNull ? null : lease;
}

class _CompletingSource implements PairingCameraSource {
  final Completer<PairingCameraLease?> _completer =
      Completer<PairingCameraLease?>();
  final _FakeLease lease = _FakeLease();

  @override
  Future<PairingCameraLease?> open() => _completer.future;

  void complete() => _completer.complete(lease);
}

class _FakeLease implements PairingCameraLease {
  void Function(QrCameraFrame frame)? _onFrame;
  bool stopped = false;
  bool disposed = false;

  @override
  Widget get preview => const SizedBox();

  @override
  Future<void> dispose() async {
    disposed = true;
  }

  void emit(QrCameraFrame frame) => _onFrame?.call(frame);

  @override
  Future<void> startFrames(void Function(QrCameraFrame frame) onFrame) async {
    _onFrame = onFrame;
  }

  @override
  Future<void> stopFrames() async {
    stopped = true;
  }
}

class _ImmediateDecoder implements QrFrameDecoder {
  @override
  Future<String?> decode(QrLuminanceFrame frame) async => 'payload';
}

class _FakeSink implements PairingQrPayloadSink {
  final List<String> payloads = <String>[];

  @override
  Future<QrPayloadDisposition> submit(String payload) async {
    payloads.add(payload);
    return QrPayloadDisposition.accepted;
  }
}
