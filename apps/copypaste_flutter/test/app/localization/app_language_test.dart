import 'package:copypaste_flutter/app/app.dart';
import 'package:copypaste_flutter/app/localization/app_language.dart';
import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('forces the stored language where the OS has no app language', () async {
    final store = MemoryAppLanguageStore(AppLanguage.ukrainian);
    final controller = AppLanguageController(store: store);
    addTearDown(controller.dispose);

    await controller.initialize();
    expect(controller.locale, const Locale('uk'));

    await controller.select(AppLanguage.system);
    expect(controller.locale, isNull);
    expect(store.value, AppLanguage.system);
  });

  test(
    'follows a live OS app language and writes choices back to it',
    () async {
      final platform = _Platform((language: AppLanguage.german, live: true));
      final controller = AppLanguageController(
        store: MemoryAppLanguageStore(AppLanguage.french),
        platform: platform,
      );
      addTearDown(controller.dispose);

      await controller.initialize();
      expect(controller.language, AppLanguage.german);
      expect(controller.locale, isNull);

      await controller.select(AppLanguage.spanish);
      expect(platform.applied, [const Locale('es')]);
      expect(controller.locale, isNull);
    },
  );

  test('forces the OS app language that applies only after relaunch', () async {
    final controller = AppLanguageController(
      store: MemoryAppLanguageStore(),
      platform: _Platform((language: AppLanguage.japanese, live: false)),
    );
    addTearDown(controller.dispose);

    await controller.initialize();
    expect(controller.locale, const Locale('ja'));
  });

  testWidgets('switches the interface language live', (tester) async {
    final navigation = AppNavigationController();
    final language = AppLanguageController(
      store: MemoryAppLanguageStore(AppLanguage.ukrainian),
    );
    addTearDown(navigation.dispose);
    addTearDown(language.dispose);
    await language.initialize();

    await tester.pumpWidget(
      ListenableBuilder(
        listenable: language,
        builder: (context, _) =>
            CopyPasteApp(navigation: navigation, language: language),
      ),
    );
    await tester.pump();
    expect(find.text('Середовище історії недоступне'), findsOneWidget);

    await language.select(AppLanguage.english);
    await tester.pump();
    expect(find.text('History runtime is unavailable'), findsOneWidget);
    expect(AppStrings.current.appDestinationHistory, 'History');
  });

  test('resolves unsupported system languages to English', () {
    expect(
      AppLocalization.resolve(const [
        Locale('ru'),
        Locale('xx'),
      ], AppLocalization.supportedLocales),
      const Locale('en'),
    );
    expect(AppLanguage.fromTag('pt-BR'), AppLanguage.portuguese);
  });
}

class _Platform implements AppLanguagePlatform {
  _Platform(this.state);

  final AppLanguagePlatformState state;
  final applied = <Locale?>[];

  @override
  Future<AppLanguagePlatformState?> read() async => state;

  @override
  Future<void> apply(Locale? locale) async => applied.add(locale);
}
