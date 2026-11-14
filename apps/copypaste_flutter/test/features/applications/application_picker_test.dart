import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/applications/controller/application_directory.dart';
import 'package:copypaste_flutter/features/applications/models/application_models.dart';
import 'package:copypaste_flutter/features/applications/repository/application_catalog.dart';
import 'package:copypaste_flutter/features/applications/view/application_picker.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

class _Catalog implements ApplicationCatalog {
  final iconRequests = <String>[];
  final lookups = <String>[];
  var seenLoads = 0;
  var cancelled = 0;
  var seenApps = const [
    ApplicationChoice(id: 'com.example.writer', name: 'writer', iconId: 'c1'),
    ApplicationChoice(id: 'org.example.Mail', name: 'Mail', iconId: 'c2'),
  ];

  @override
  Future<List<ApplicationChoice>> installed() async => const [
    ApplicationChoice(id: 'com.example.writer', name: 'Writer'),
    ApplicationChoice(id: 'com.example.atlas', name: 'Atlas'),
    ApplicationChoice(id: 'com.example.vault', name: 'Vault'),
  ];

  @override
  Future<ApplicationChoice?> installedById(String id) async {
    lookups.add(id);
    return (await installed()).where((app) => app.id == id).firstOrNull;
  }

  @override
  Future<List<ApplicationChoice>> seen() async {
    seenLoads++;
    return seenApps;
  }

  @override
  void cancelIcons() => cancelled++;

  @override
  Future<HistorySourceAppIcon?> installedIcon(String id) async {
    iconRequests.add(id);
    return null;
  }

  @override
  Future<HistorySourceAppIcon?> seenIcon(String iconId) async {
    iconRequests.add(iconId);
    return null;
  }
}

void main() {
  test('candidates merge installed and seen apps once each by name', () async {
    final directory = ApplicationDirectory(_Catalog());
    final apps = await directory.candidates(exclude: ['com.example.vault']);
    expect(
      [for (final app in apps) (app.id, app.name, app.iconId)],
      [
        ('com.example.atlas', 'Atlas', null),
        ('org.example.Mail', 'Mail', 'c2'),
        ('com.example.writer', 'Writer', 'c1'),
      ],
    );
    expect(await directory.describe('com.example.gone'), isNull);
  });

  test('Windows image names match without case or extension', () {
    final directory = ApplicationDirectory(
      _Catalog(),
      platform: TargetPlatform.windows,
    );
    final apps = directory.merge(
      installed: const [ApplicationChoice(id: 'editor.exe', name: 'Editor')],
      seen: const [ApplicationChoice(id: 'Editor.exe', name: 'Editor')],
      exclude: const ['EDITOR'],
    );
    expect(apps, isEmpty);
  });

  test('only seen apps with this platform\'s identifier form are offered', () {
    const seen = [
      ApplicationChoice(id: 'zoom.exe', name: 'Zoom'),
      ApplicationChoice(id: 'com.example.notes', name: 'Notes'),
      ApplicationChoice(id: 'org.gnome.gedit-beta', name: 'Gedit'),
      ApplicationChoice(id: 'firefox', name: 'Firefox'),
    ];
    List<String> offered(TargetPlatform platform) => [
      for (final app in ApplicationDirectory(
        _Catalog(),
        platform: platform,
      ).merge(installed: const [], seen: seen))
        app.id,
    ];
    expect(offered(TargetPlatform.windows), ['zoom.exe']);
    expect(offered(TargetPlatform.macOS), [
      'org.gnome.gedit-beta',
      'com.example.notes',
    ]);
    expect(offered(TargetPlatform.android), ['com.example.notes']);
    expect(offered(TargetPlatform.linux), [
      'firefox',
      'org.gnome.gedit-beta',
      'com.example.notes',
    ]);
  });

  test('misses are retried and History is released after lookups', () async {
    final catalog = _Catalog();
    var now = DateTime(2026);
    final directory = ApplicationDirectory(catalog, clock: () => now);

    final (writer, gone) = await (
      directory.describe('com.example.writer'),
      directory.describe('com.example.gone'),
    ).wait;
    expect(writer?.name, 'Writer');
    expect(gone, isNull);
    expect(catalog.seenLoads, 1);

    expect(await directory.describe('com.example.gone'), isNull);
    expect(
      catalog.lookups.where((id) => id == 'com.example.gone'),
      hasLength(1),
    );

    now = now.add(ApplicationDirectory.missLifetime);
    catalog.seenApps = const [
      ApplicationChoice(id: 'com.example.gone', name: 'Back'),
    ];
    expect((await directory.describe('com.example.gone'))?.name, 'Back');
    expect(catalog.seenLoads, 2);
    expect((await directory.describe('com.example.writer'))?.name, 'Writer');
    expect(
      catalog.lookups.where((id) => id == 'com.example.writer'),
      hasLength(1),
    );
  });

  testWidgets('picking an application resolves to its identifier', (
    tester,
  ) async {
    final catalog = _Catalog();
    ApplicationChoice? picked;
    await tester.pumpWidget(
      ShadcnApp(
        localizationsDelegates: AppLocalization.delegates,
        supportedLocales: AppLocalization.supportedLocales,
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: Builder(
            builder: (context) => Center(
              child: Button.primary(
                onPressed: () async => picked = await showApplicationPicker(
                  context,
                  catalog: catalog,
                  exclude: const ['com.example.atlas'],
                ),
                child: const Text('Open'),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();

    expect(find.textContaining('Atlas', findRichText: true), findsNothing);
    expect(find.textContaining('Writer', findRichText: true), findsOneWidget);
    expect(catalog.iconRequests, containsAll(['c1', 'c2']));

    await tester.enterText(find.byType(TextField), 'mai');
    await tester.pumpAndSettle();
    expect(find.textContaining('Writer', findRichText: true), findsNothing);

    await tester.tap(find.textContaining('Mail', findRichText: true));
    await tester.pumpAndSettle();
    expect(picked?.id, 'org.example.Mail');
    expect(catalog.cancelled, 1);
  });
}
