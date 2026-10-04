import 'dart:async';

import 'package:camera/camera.dart';
import 'package:flutter/widgets.dart';

import 'qr_frame.dart';
import 'qr_scanner.dart';

enum PairingCameraState {
  idle,
  requestingPermission,
  scanning,
  unavailable,
  permissionDenied,
  failed,
}

/// A camera source hides plugin controller ownership from the device screen.
abstract interface class PairingCameraSource {
  Future<PairingCameraLease?> open();
}

/// The preview is the maintained plugin widget. No camera preview wrapper is
/// introduced by this adapter.
abstract interface class PairingCameraLease {
  Widget get preview;

  Future<void> startFrames(void Function(QrCameraFrame frame) onFrame);

  Future<void> stopFrames();

  Future<void> dispose();
}

/// Camera lifecycle and QR decoding coordinator for a device-pairing scanner.
/// It closes streams whenever the app becomes inactive and drops late frame
/// callbacks by session token.
class PairingCameraScanner with WidgetsBindingObserver {
  PairingCameraScanner({
    required PairingCameraSource source,
    required QrScanCoordinator Function(void Function(QrScanProgress))
    coordinatorFactory,
  }) : _source = source,
       _coordinatorFactory = coordinatorFactory;

  final PairingCameraSource _source;
  final QrScanCoordinator Function(void Function(QrScanProgress))
  _coordinatorFactory;
  final ValueNotifier<PairingCameraState> state = ValueNotifier(
    PairingCameraState.idle,
  );
  final ValueNotifier<QrScanProgress> scanProgress = ValueNotifier(
    const QrScanProgress(QrScanStatus.idle),
  );

  PairingCameraLease? _lease;
  QrScanCoordinator? _coordinator;
  bool _desired = false;
  bool _disposed = false;
  bool _starting = false;
  bool _observingLifecycle = false;
  int _session = 0;

  Widget? get preview => _lease?.preview;

  Future<void> start() async {
    _desired = true;
    observeLifecycle();
    if (_disposed || _starting || _lease != null) {
      return;
    }
    _starting = true;
    final session = ++_session;
    state.value = PairingCameraState.requestingPermission;
    try {
      final lease = await _source.open();
      if (_isStale(session) || lease == null) {
        await lease?.dispose();
        if (!_disposed && lease == null) {
          state.value = PairingCameraState.unavailable;
        }
        return;
      }
      final coordinator = _coordinatorFactory((event) {
        if (!_isStale(session)) {
          scanProgress.value = event;
        }
      });
      _lease = lease;
      _coordinator = coordinator;
      await lease.startFrames((frame) {
        if (!_isStale(session)) {
          unawaited(coordinator.submit(frame));
        }
      });
      if (_isStale(session)) {
        await _closeSession();
        return;
      }
      state.value = PairingCameraState.scanning;
    } on CameraException catch (error) {
      if (!_isStale(session)) {
        state.value = _isPermissionError(error)
            ? PairingCameraState.permissionDenied
            : PairingCameraState.failed;
      }
    } on Object {
      if (!_isStale(session)) {
        state.value = PairingCameraState.failed;
      }
    } finally {
      _starting = false;
    }
  }

  Future<void> stop() async {
    _desired = false;
    await _closeSession();
    if (!_disposed) {
      state.value = PairingCameraState.idle;
      scanProgress.value = const QrScanProgress(QrScanStatus.idle);
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    switch (state) {
      case AppLifecycleState.resumed:
        if (_desired) {
          unawaited(start());
        }
        return;
      case AppLifecycleState.inactive:
      case AppLifecycleState.paused:
      case AppLifecycleState.detached:
      case AppLifecycleState.hidden:
        unawaited(_closeSession());
    }
  }

  Future<void> dispose() async {
    if (_disposed) {
      return;
    }
    _disposed = true;
    _desired = false;
    if (_observingLifecycle) {
      WidgetsBinding.instance.removeObserver(this);
      _observingLifecycle = false;
    }
    await _closeSession();
    state.dispose();
    scanProgress.dispose();
  }

  void observeLifecycle() {
    if (_observingLifecycle) {
      return;
    }
    WidgetsBinding.instance.addObserver(this);
    _observingLifecycle = true;
  }

  Future<void> _closeSession() async {
    _session++;
    final coordinator = _coordinator;
    _coordinator = null;
    coordinator?.close();
    final lease = _lease;
    _lease = null;
    if (lease == null) {
      return;
    }
    try {
      await lease.stopFrames();
    } on Object {
      // Disposing the controller is still required after a stream failure.
    }
    await lease.dispose();
  }

  bool _isStale(int session) => _disposed || !_desired || session != _session;

  bool _isPermissionError(CameraException error) => switch (error.code) {
    'CameraAccessDenied' ||
    'CameraAccessDeniedWithoutPrompt' ||
    'CameraAccessRestricted' => true,
    _ => false,
  };
}

/// Production plugin source for macOS, Android, and Windows. The package's
/// desktop implementation supplies BGRA frames; Android supplies YUV or NV21.
class CameraPluginPairingSource implements PairingCameraSource {
  const CameraPluginPairingSource();

  @override
  Future<PairingCameraLease?> open() async {
    final cameras = await availableCameras();
    if (cameras.isEmpty) {
      return null;
    }
    final camera = _preferredCamera(cameras);
    final controller = CameraController(
      camera,
      ResolutionPreset.medium,
      enableAudio: false,
    );
    try {
      await controller.initialize();
      return _CameraPluginPairingLease(controller);
    } on Object {
      await controller.dispose();
      rethrow;
    }
  }

  CameraDescription _preferredCamera(List<CameraDescription> cameras) {
    for (final camera in cameras) {
      if (camera.lensDirection == CameraLensDirection.back) {
        return camera;
      }
    }
    return cameras.first;
  }
}

class _CameraPluginPairingLease implements PairingCameraLease {
  _CameraPluginPairingLease(this._controller);

  final CameraController _controller;

  @override
  Widget get preview => CameraPreview(_controller);

  @override
  Future<void> startFrames(void Function(QrCameraFrame frame) onFrame) {
    return _controller.startImageStream((image) {
      final frame = _toQrFrame(image, _controller.description);
      if (frame != null) {
        onFrame(frame);
      }
    });
  }

  @override
  Future<void> stopFrames() async {
    if (_controller.value.isStreamingImages) {
      await _controller.stopImageStream();
    }
  }

  @override
  Future<void> dispose() => _controller.dispose();

  QrCameraFrame? _toQrFrame(CameraImage image, CameraDescription camera) {
    final format = switch (image.format.group) {
      ImageFormatGroup.yuv420 => QrFrameFormat.yuv420,
      ImageFormatGroup.nv21 => QrFrameFormat.nv21,
      ImageFormatGroup.bgra8888 => QrFrameFormat.bgra8888,
      _ => null,
    };
    if (format == null || image.planes.isEmpty) {
      return null;
    }
    return QrCameraFrame(
      width: image.width,
      height: image.height,
      format: format,
      rotationDegrees: camera.sensorOrientation,
      mirrored: camera.lensDirection == CameraLensDirection.front,
      planes: image.planes
          .map(
            (plane) => QrFramePlane(
              bytes: plane.bytes,
              bytesPerRow: plane.bytesPerRow,
              bytesPerPixel:
                  plane.bytesPerPixel ??
                  (format == QrFrameFormat.bgra8888 ? 4 : 1),
            ),
          )
          .toList(growable: false),
    );
  }
}
