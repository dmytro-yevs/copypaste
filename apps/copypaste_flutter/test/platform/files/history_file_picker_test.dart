import 'package:copypaste_flutter/platform/files/history_file_picker.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const channel = SystemHistoryFilePicker.androidChannel;
  late List<MethodCall> calls;

  TestWidgetsFlutterBinding.ensureInitialized();

  void choose({required int? sizeBytes, bool releaseFails = false}) {
    calls = [];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call);
          return switch (call.method) {
            'chooseFiles' => [
              {'token': 't', 'name': 'a.cpmodule', 'sizeBytes': sizeBytes},
            ],
            'materialize' => (call.arguments as Map)['destination'],
            'release' when releaseFails => throw PlatformException(code: 'x'),
            _ => null,
          };
        });
  }

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  test('rejects a document over the limit before copying any byte', () async {
    choose(sizeBytes: 11);
    var destinationRequested = false;

    await expectLater(
      SystemHistoryFilePicker.copyChosenAndroidDocument(
        maxBytes: 10,
        destination: (_) async {
          destinationRequested = true;
          return '/cache/input';
        },
      ),
      throwsA(
        isA<PlatformFileException>().having(
          (e) => e.tooLarge,
          'tooLarge',
          true,
        ),
      ),
    );
    expect(destinationRequested, isFalse);
    expect(calls.map((call) => call.method), ['chooseFiles', 'release']);
    expect(calls.first.arguments, {'oneShot': true});
  });

  test('streams a fitting document to the caller-owned path', () async {
    choose(sizeBytes: 10);

    final copy = await SystemHistoryFilePicker.copyChosenAndroidDocument(
      maxBytes: 10,
      destination: (name) async => '/cache/$name',
    );

    expect(copy, (name: 'a.cpmodule', path: '/cache/a.cpmodule'));
    expect(calls.map((call) => call.method), [
      'chooseFiles',
      'materialize',
      'release',
    ]);
    expect(calls[1].arguments, {
      'token': 't',
      'maxBytes': 10,
      'destination': '/cache/a.cpmodule',
    });
  });

  test('a failed release does not replace the copy result', () async {
    choose(sizeBytes: 10, releaseFails: true);

    final copy = await SystemHistoryFilePicker.copyChosenAndroidDocument(
      destination: (name) async => '/cache/$name',
    );

    expect(copy?.path, '/cache/a.cpmodule');
    expect(calls.last.method, 'release');
  });
}
