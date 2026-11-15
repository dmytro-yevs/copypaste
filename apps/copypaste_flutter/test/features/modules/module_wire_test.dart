import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_view.dart';
import 'package:copypaste_flutter/features/modules/repository/file_selector_module_file_saver.dart';
import 'package:copypaste_flutter/features/modules/repository/module_wire.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('schema 7 surfaces, views and outputs parse', () {
    final surfaces = ModuleWire.surfaces({
      'settings': {'command': 'library', 'icon': 'notebook_text'},
      'quick_paste': {
        'title': 'Library',
        'search_command': 'search',
        'activate_command': 'use',
        'mixed_limit': 5,
      },
      'item_actions': [
        {'id': 'save', 'title': 'Save', 'command': 'save-item', 'icon': 'x'},
      ],
      'tray_actions': [
        {'id': 'new', 'title': 'New', 'command': 'new'},
      ],
    });
    expect(surfaces.settings?.icon, ModuleIcon.notebookText);
    expect(surfaces.quickPaste?.mixedLimit, 5);
    // An icon this app does not know falls back to the host default.
    expect(surfaces.itemActions.single.icon, isNull);
    expect(surfaces.trayActions.single.command, 'new');

    final view = ModuleWire.output({
      'kind': 'view',
      'view': {
        'sections': [
          {
            'title': null,
            'items': [
              {
                'kind': 'search',
                'query': '',
                'placeholder': 'Search',
                'action': {'title': 'Search', 'command': 'library'},
              },
              {
                'kind': 'row',
                'id': 's1',
                'title': 'Address',
                'subtitle': 'Main St',
                'badges': [';addr'],
                'secret': true,
                'open': {
                  'title': 'Edit',
                  'command': 'edit',
                  'target': 'drawer',
                  'require_auth': true,
                },
              },
              {
                'kind': 'form',
                'fields': [
                  {
                    'id': 'title',
                    'title': 'Title',
                    'kind': 'text',
                    'default': '',
                  },
                  {
                    'id': 'body',
                    'title': 'Body',
                    'kind': 'text',
                    'default': '',
                    'multiline': true,
                    'template': true,
                    'max_bytes': 262144,
                  },
                ],
                'values': {'body': 'Hi {{name}}'},
                'errors': {'body': 'Unknown field'},
                'submit': {'title': 'Save', 'command': 'save'},
              },
              {'kind': 'future_item'},
            ],
            'bulk_actions': [
              {'title': 'Delete', 'command': 'delete', 'destructive': true},
            ],
            'reorder': {'title': 'Move', 'command': 'move'},
          },
        ],
      },
    }).view!;
    final section = view.sections.single;
    expect(section.items, hasLength(3));
    final row = section.items[1] as ModuleViewRow;
    expect(row.secret, isTrue);
    expect(row.open?.target, ModuleViewTarget.drawer);
    expect(row.open?.requireAuth, isTrue);
    final form = section.items[2] as ModuleViewForm;
    expect(form.fields.first.kind, ModuleFieldKind.line);
    expect(form.fields.last.kind, ModuleFieldKind.text);
    expect(form.fields.last.template, isTrue);
    expect(form.fields.last.maxBytes, 262144);
    expect(form.errors, {'body': 'Unknown field'});
    expect(section.bulkActions.single.destructive, isTrue);
    expect(section.reorder?.command, 'move');

    final results = ModuleWire.output({
      'kind': 'results',
      'items': [
        {'id': 's1', 'title': 'Address', 'preview': '', 'preselect': true},
      ],
    }).results!;
    expect(results.single.preselect, isTrue);
    final delivery = ModuleWire.output({
      'kind': 'deliver',
      'delivery': {
        'text': 'Main St',
        'label': 'Snippet',
        'record_history': true,
        'cursor_graphemes_from_end': 3,
      },
    }).delivery!;
    expect(delivery.recordHistory, isTrue);
    expect(delivery.cursorFromEnd, 3);
    expect(
      ModuleWire.output({
        'kind': 'save_file',
        'path': 'exports/a.json',
        'name': 'a.json',
      }).file?.name,
      'a.json',
    );
    expect(
      ModuleWire.output({'kind': 'message', 'message': 'Saved'}).message,
      isTrue,
    );
  });

  test('saved module files stay inside the module data directory', () {
    expect(moduleFilePath('/data', 'exports/a.json'), endsWith('a.json'));
    for (final path in ['../a', '/etc/passwd', 'a/../../b', '']) {
      expect(
        () => moduleFilePath('/data', path),
        throwsA(isA<ModulesException>()),
      );
    }
  });
}
