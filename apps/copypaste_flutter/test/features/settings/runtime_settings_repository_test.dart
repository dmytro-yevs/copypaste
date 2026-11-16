import 'package:copypaste_flutter/features/settings/repository/runtime_settings_repository.dart';
import 'package:copypaste_flutter/generated/api.dart';
import 'package:copypaste_flutter/generated/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('the capture preview reads the bounded row, never the body', () async {
    final api = _PreviewApi();
    RustLib.initMock(api: api);
    addTearDown(RustLib.dispose);

    final preview = await RuntimeSettingsRepository().capturePreview('clip');

    expect(api.previewed, ['clip']);
    expect(preview?.text, 'row preview');
  });
}

class _PreviewApi implements RustLibApi {
  final previewed = <String>[];

  @override
  Future<Clip> crateApiClipsClipPreview({required String id}) async {
    previewed.add(id);
    return Clip(
      secret: false,
      transient: false,
      id: id,
      content: 'row preview',
      contentType: 'text',
      contentClass: ClipContentClass.text,
      createdAtMs: 0,
      pinned: false,
      tagIds: const [],
      originDeviceClass: DeviceClass.unknown,
      truncated: true,
      tooLargeToSync: false,
    );
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw StateError('unexpected ${invocation.memberName}');
}
