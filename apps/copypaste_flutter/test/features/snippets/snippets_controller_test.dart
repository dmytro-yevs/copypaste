import 'package:copypaste_flutter/features/snippets/controller/concealed_snippet_access.dart';
import 'package:copypaste_flutter/features/snippets/controller/snippets_controller.dart';
import 'package:copypaste_flutter/features/snippets/models/snippet_models.dart';
import 'package:copypaste_flutter/platform/security/device_authenticator.dart';
import 'package:flutter_test/flutter_test.dart';

import 'snippet_test_support.dart';

class _Authenticator implements DeviceAuthenticator {
  _Authenticator(this.result);

  final DeviceAuthResult result;
  int prompts = 0;

  @override
  Future<DeviceAuthAvailability> availability() async =>
      DeviceAuthAvailability.available;

  @override
  Future<DeviceAuthResult> authenticate({required String reason}) async {
    prompts++;
    return result;
  }
}

Future<void> _settle() => Future<void>.delayed(Duration.zero);

void main() {
  test('pinned snippets lead the list once, and detaching drops the loaded '
      'content and releases the runtime cache', () async {
    final repository = FakeSnippetRepository(
      snippets: [
        summary('a', title: 'Address'),
        summary('b', title: 'Bio', pinned: true),
      ],
    );
    final controller = SnippetsController(repository: repository);
    addTearDown(controller.dispose);

    controller.attach();
    await _settle();
    expect(controller.status, SnippetsStatus.ready);
    expect(controller.pinned.map((row) => row.id), ['b']);
    expect(controller.rows.map((row) => row.id), ['a']);

    controller.detach();
    expect(controller.status, SnippetsStatus.idle);
    expect(controller.rows, isEmpty);
    expect(controller.pinned, isEmpty);
    expect(repository.releases, 1);
  });

  test('a concealed snippet needs the owner while require_auth is on, and '
      'one success opens a window that detaching closes', () async {
    final authenticator = _Authenticator(DeviceAuthResult.success);
    final access = ConcealedSnippetAccess(authenticator: authenticator);
    final repository = FakeSnippetRepository(
      snippets: [summary('c', concealed: true), summary('p')],
      preferencesValue: const SnippetPreferences(requireAuth: true),
    );
    final controller = SnippetsController(
      repository: repository,
      access: access,
    );
    addTearDown(controller.dispose);
    controller.attach();
    await _settle();

    final concealed = repository.snippets.first;
    expect(controller.needsUnlock(concealed), isTrue);
    expect(controller.needsUnlock(repository.snippets.last), isFalse);

    expect(await access.request('reason'), ConcealedAccess.granted);
    expect(await access.request('reason'), ConcealedAccess.granted);
    expect(authenticator.prompts, 1);
    expect(controller.needsUnlock(concealed), isFalse);

    controller.detach();
    expect(access.unlocked, isFalse);
  });
}
