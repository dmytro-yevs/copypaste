import 'dart:typed_data';

enum QrFrameFormat { yuv420, nv21, bgra8888 }

class QrFramePlane {
  const QrFramePlane({
    required this.bytes,
    required this.bytesPerRow,
    required this.bytesPerPixel,
  });

  final Uint8List bytes;
  final int bytesPerRow;
  final int bytesPerPixel;
}

/// A platform-neutral frame shape used by the QR scanner. The camera adapter is
/// responsible for mapping plugin frames into this immutable value.
class QrCameraFrame {
  const QrCameraFrame({
    required this.width,
    required this.height,
    required this.format,
    required this.planes,
    this.rotationDegrees = 0,
    this.mirrored = false,
  });

  final int width;
  final int height;
  final QrFrameFormat format;
  final List<QrFramePlane> planes;
  final int rotationDegrees;
  final bool mirrored;
}

class QrLuminanceFrame {
  const QrLuminanceFrame({
    required this.width,
    required this.height,
    required this.bytes,
  });

  final int width;
  final int height;
  final Int8List bytes;
}

/// Converts camera formats into a bounded, oriented luminance image for one
/// decoder attempt. It honors row stride for desktop BGRA frames and Android
/// YUV/NV21 planes.
class QrFrameNormalizer {
  const QrFrameNormalizer({this.maximumDimension = 960});

  final int maximumDimension;

  QrLuminanceFrame normalize(QrCameraFrame frame) {
    if (frame.width <= 0 || frame.height <= 0 || maximumDimension <= 0) {
      throw ArgumentError('QR frame dimensions must be positive.');
    }
    if (frame.planes.isEmpty) {
      throw ArgumentError('QR frame has no image planes.');
    }
    final rotation = frame.rotationDegrees % 360;
    if (rotation != 0 && rotation != 90 && rotation != 180 && rotation != 270) {
      throw ArgumentError(
        'QR frame rotation must be a multiple of 90 degrees.',
      );
    }

    final rotatedWidth = rotation == 90 || rotation == 270
        ? frame.height
        : frame.width;
    final rotatedHeight = rotation == 90 || rotation == 270
        ? frame.width
        : frame.height;
    final scale = maximumDimension / _max(rotatedWidth, rotatedHeight);
    final width = scale < 1 ? (rotatedWidth * scale).round() : rotatedWidth;
    final height = scale < 1 ? (rotatedHeight * scale).round() : rotatedHeight;
    final output = Int8List(width * height);
    final plane = frame.planes.first;

    for (var y = 0; y < height; y++) {
      final rotatedY = (y * rotatedHeight ~/ height)
          .clamp(0, rotatedHeight - 1)
          .toInt();
      for (var x = 0; x < width; x++) {
        var rotatedX = (x * rotatedWidth ~/ width)
            .clamp(0, rotatedWidth - 1)
            .toInt();
        if (frame.mirrored) {
          rotatedX = rotatedWidth - 1 - rotatedX;
        }
        final source = _sourceCoordinates(
          x: rotatedX,
          y: rotatedY,
          width: frame.width,
          height: frame.height,
          rotation: rotation,
        );
        output[y * width + x] = _luminanceAt(
          frame: frame,
          plane: plane,
          x: source.$1,
          y: source.$2,
        );
      }
    }
    return QrLuminanceFrame(width: width, height: height, bytes: output);
  }

  (int, int) _sourceCoordinates({
    required int x,
    required int y,
    required int width,
    required int height,
    required int rotation,
  }) {
    return switch (rotation) {
      0 => (x, y),
      90 => (y, height - 1 - x),
      180 => (width - 1 - x, height - 1 - y),
      270 => (width - 1 - y, x),
      _ => throw StateError('Unsupported QR frame rotation.'),
    };
  }

  int _luminanceAt({
    required QrCameraFrame frame,
    required QrFramePlane plane,
    required int x,
    required int y,
  }) {
    final offset = y * plane.bytesPerRow + x * plane.bytesPerPixel;
    final requiredBytes = switch (frame.format) {
      QrFrameFormat.yuv420 || QrFrameFormat.nv21 => 1,
      QrFrameFormat.bgra8888 => 4,
    };
    if (plane.bytesPerRow <= 0 ||
        plane.bytesPerPixel < requiredBytes ||
        offset < 0 ||
        offset + requiredBytes > plane.bytes.length) {
      throw ArgumentError(
        'QR frame plane has invalid row stride or pixel data.',
      );
    }
    return switch (frame.format) {
      QrFrameFormat.yuv420 || QrFrameFormat.nv21 => plane.bytes[offset],
      QrFrameFormat.bgra8888 =>
        ((plane.bytes[offset + 2] * 77 +
                plane.bytes[offset + 1] * 150 +
                plane.bytes[offset] * 29) >>
            8),
    };
  }

  int _max(int first, int second) => first > second ? first : second;
}
