import 'dart:typed_data';

import 'package:copypaste_flutter/platform/camera/qr_frame.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('uses bytesPerRow when normalizing padded BGRA frames', () {
    final frame = QrCameraFrame(
      width: 2,
      height: 2,
      format: QrFrameFormat.bgra8888,
      planes: <QrFramePlane>[
        QrFramePlane(
          bytes: Uint8List.fromList(<int>[
            0,
            0,
            0,
            255,
            255,
            255,
            255,
            255,
            9,
            9,
            9,
            9,
            255,
            0,
            0,
            255,
            0,
            255,
            0,
            255,
            9,
            9,
            9,
            9,
          ]),
          bytesPerRow: 12,
          bytesPerPixel: 4,
        ),
      ],
    );

    final normalized = const QrFrameNormalizer().normalize(frame);

    expect(normalized.bytes.map((value) => value & 0xff), <int>[
      0,
      255,
      28,
      149,
    ]);
  });

  test('reads Y plane for Android YUV and NV21 frames', () {
    for (final format in <QrFrameFormat>[
      QrFrameFormat.yuv420,
      QrFrameFormat.nv21,
    ]) {
      final frame = QrCameraFrame(
        width: 2,
        height: 2,
        format: format,
        planes: <QrFramePlane>[
          QrFramePlane(
            bytes: Uint8List.fromList(<int>[1, 2, 88, 3, 4, 99]),
            bytesPerRow: 3,
            bytesPerPixel: 1,
          ),
        ],
      );

      expect(const QrFrameNormalizer().normalize(frame).bytes, <int>[
        1,
        2,
        3,
        4,
      ]);
    }
  });

  test('normalizes rotation and mirrored frames before decoding', () {
    final frame = QrCameraFrame(
      width: 2,
      height: 3,
      format: QrFrameFormat.yuv420,
      rotationDegrees: 90,
      mirrored: true,
      planes: <QrFramePlane>[
        QrFramePlane(
          bytes: Uint8List.fromList(<int>[1, 2, 3, 4, 5, 6]),
          bytesPerRow: 2,
          bytesPerPixel: 1,
        ),
      ],
    );

    final normalized = const QrFrameNormalizer().normalize(frame);

    expect(normalized.width, 3);
    expect(normalized.height, 2);
    expect(normalized.bytes, <int>[1, 3, 5, 2, 4, 6]);
  });

  test('bounds the longest dimension before a decoder attempt', () {
    final frame = QrCameraFrame(
      width: 8,
      height: 4,
      format: QrFrameFormat.yuv420,
      planes: <QrFramePlane>[
        QrFramePlane(bytes: Uint8List(32), bytesPerRow: 8, bytesPerPixel: 1),
      ],
    );

    final normalized = const QrFrameNormalizer(
      maximumDimension: 4,
    ).normalize(frame);

    expect(normalized.width, 4);
    expect(normalized.height, 2);
  });
}
