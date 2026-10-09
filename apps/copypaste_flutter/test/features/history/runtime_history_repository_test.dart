import 'dart:convert';

import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/repository/runtime_history_repository.dart';
import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:flutter_test/flutter_test.dart';

void main() {
  test(
    'maps generated list and detail clips without duplicating the wire model',
    () {
      final generated = runtime.Clip(
        secret: false,
        transient: false,
        id: 'clip-1',
        content: 'The complete clip body',
        contentType: 'file',
        contentClass: runtime.ClipContentClass.file,
        createdAtMs: 1704067200000,
        pinned: true,
        originDeviceName: 'Work laptop',
        originDeviceClass: runtime.DeviceClass.laptop,
        sourceAppName: 'Editor',
        sourceAppIconId: 'app:editor',
        truncated: false,
        tooLargeToSync: false,
        fileDetails: runtime.ClipFileDetails(
          filename: 'report.pdf',
          mimeType: 'application/pdf',
          sourceReference: '/Users/person/Documents/report.pdf',
          sourceAvailable: false,
          sizeBytes: BigInt.from(2048),
          fileCount: 1,
        ),
      );

      final listed = RuntimeHistoryRepository.mapListClip(generated);
      final detail = RuntimeHistoryRepository.mapDetailClip(generated);

      expect(listed.contentKind, HistoryClipKind.file);
      expect(listed.body, isNull);
      expect(detail.body, 'The complete clip body');
      expect(detail.file?.name, 'report.pdf');
      expect(detail.file?.mimeType, 'application/pdf');
      expect(
        detail.file?.sourceReference,
        '/Users/person/Documents/report.pdf',
      );
      expect(detail.file?.sourceAvailable, isFalse);
      expect(detail.file?.sizeBytes, 2048);
      expect(detail.file?.fileCount, 1);
      expect(detail.origin, 'Work laptop');
      expect(detail.originDeviceClass, DeviceClass.laptop);
      expect(detail.sourceApp, 'Editor');
      expect(detail.createdAt, DateTime.utc(2024));
      expect(detail.tooLargeToSync, isFalse);
    },
  );

  test('maps original image metadata and sync refusal state', () {
    final detail = RuntimeHistoryRepository.mapDetailClip(
      runtime.Clip(
        secret: false,
        transient: false,
        id: 'image-1',
        content: '[image]',
        contentType: 'image/png',
        contentClass: runtime.ClipContentClass.image,
        createdAtMs: 0,
        pinned: false,
        originDeviceName: 'Phone',
        originDeviceClass: runtime.DeviceClass.phone,
        truncated: false,
        tooLargeToSync: true,
        imageDetails: runtime.ClipImageDetails(
          width: 1920,
          height: 1080,
          sizeBytes: BigInt.from(4 * 1024 * 1024),
        ),
      ),
    );

    expect(detail.contentKind, HistoryClipKind.image);
    expect(detail.origin, 'Phone');
    expect(detail.originDeviceClass, DeviceClass.phone);
    expect(detail.image?.width, 1920);
    expect(detail.image?.height, 1080);
    expect(detail.image?.sizeBytes, 4 * 1024 * 1024);
    expect(detail.tooLargeToSync, isTrue);
  });

  test('maps every generated semantic kind and its color swatch', () {
    final expected = <runtime.ClipSemanticKind, HistoryClipKind>{
      runtime.ClipSemanticKind.plainText: HistoryClipKind.text,
      runtime.ClipSemanticKind.link: HistoryClipKind.link,
      runtime.ClipSemanticKind.email: HistoryClipKind.email,
      runtime.ClipSemanticKind.color: HistoryClipKind.color,
      runtime.ClipSemanticKind.phone: HistoryClipKind.phone,
      runtime.ClipSemanticKind.code: HistoryClipKind.code,
      runtime.ClipSemanticKind.json: HistoryClipKind.json,
      runtime.ClipSemanticKind.path: HistoryClipKind.path,
    };

    for (final entry in expected.entries) {
      final clip = RuntimeHistoryRepository.mapListClip(
        runtime.Clip(
          secret: false,
          transient: false,
          id: entry.key.name,
          content: 'value',
          contentType: 'text',
          contentClass: runtime.ClipContentClass.text,
          semanticKind: entry.key,
          colorRgba: entry.key == runtime.ClipSemanticKind.color
              ? 0x11223344
              : null,
          createdAtMs: 0,
          pinned: false,
          originDeviceClass: runtime.DeviceClass.unknown,
          truncated: false,
          tooLargeToSync: false,
        ),
      );

      expect(clip.contentKind, entry.value);
      expect(
        clip.colorRgba,
        entry.key == runtime.ClipSemanticKind.color ? 0x11223344 : isNull,
      );
    }
  });

  test(
    'decodes generated image preview bytes once at the repository boundary',
    () {
      final preview = runtime.ClipImagePreview(
        pngBase64: base64Encode([1, 2, 3]),
        width: 1,
        height: 1,
      );

      final mapped = RuntimeHistoryRepository.mapImagePreview(preview);
      expect(mapped.bytes, [1, 2, 3]);
      expect(mapped.width, 1);
      expect(mapped.height, 1);
    },
  );

  test('maps opaque filter IDs separately from display labels', () {
    final facets = RuntimeHistoryRepository.mapFacets(
      const runtime.HistoryFacets(
        originDevices: [
          runtime.HistoryDeviceFacet(
            id: 'device-1',
            label: 'Work Mac',
            deviceClass: runtime.DeviceClass.laptop,
          ),
        ],
        sourceApps: [
          runtime.HistorySourceAppFacet(
            id: 'com.example.editor',
            label: 'Editor',
            iconId: 'clip-with-editor-icon',
          ),
        ],
      ),
    );

    expect(facets.originDevices.single.id, 'device-1');
    expect(facets.originDevices.single.label, 'Work Mac');
    expect(facets.originDevices.single.deviceClass, DeviceClass.laptop);
    expect(facets.sourceApps.single.id, 'com.example.editor');
    expect(facets.sourceApps.single.label, 'Editor');
    expect(facets.sourceApps.single.iconId, 'clip-with-editor-icon');
  });
}
