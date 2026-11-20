import 'dart:convert';

import 'package:copypaste_flutter/features/modules/repository/runtime_modules_repository.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('an unreadable module stays listed for removal', () {
    Map<String, Object> module(String id, {List<String> events = const []}) => {
      'id': id,
      'title': 'Module $id',
      'description': 'A module.',
      'version': '1.0.0',
      'enabled': true,
      'size_bytes': 10,
      'events': events,
      'commands': <Object>[],
      'preference_fields': <Object>[],
      'preferences': <String, Object>{},
    };

    final modules = RuntimeModulesRepository.decodeList(
      jsonEncode([
        module('copypaste.text'),
        module('copypaste.future', events: ['future_event']),
        {'title': 'No id'},
      ]),
    );

    expect(modules.map((module) => module.id), [
      'copypaste.text',
      'copypaste.future',
    ]);
    expect(modules.first.error, isNull);
    expect(modules.last.title, 'Module copypaste.future');
    expect(modules.last.error, 'The module uses an unsupported event.');
    expect(modules.last.active, isFalse);
  });
}
