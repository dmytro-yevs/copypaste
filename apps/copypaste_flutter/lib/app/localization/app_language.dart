import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Interface languages offered by the language setting.
enum AppLanguage {
  system(null, ''),
  english(Locale('en'), 'English'),
  ukrainian(Locale('uk'), 'Українська'),
  spanish(Locale('es'), 'Español'),
  german(Locale('de'), 'Deutsch'),
  french(Locale('fr'), 'Français'),
  portuguese(Locale('pt', 'BR'), 'Português (Brasil)'),
  italian(Locale('it'), 'Italiano'),
  polish(Locale('pl'), 'Polski'),
  turkish(Locale('tr'), 'Türkçe'),
  japanese(Locale('ja'), '日本語'),
  korean(Locale('ko'), '한국어'),
  chinese(Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hans'), '简体中文');

  const AppLanguage(this.locale, this.nativeName);

  /// The forced interface locale, or null to follow the system.
  final Locale? locale;

  /// The language's own name, left untranslated so people can find it.
  final String nativeName;

  /// Matches a BCP 47 tag by language, so `pt` and `pt-BR` both resolve.
  static AppLanguage fromTag(String? tag) {
    final code = tag?.split(RegExp('[-_,]')).first.toLowerCase();
    return values.firstWhere(
      (language) => language.locale?.languageCode == code,
      orElse: () => system,
    );
  }
}

abstract interface class AppLanguageStore {
  Future<AppLanguage> read();
  Future<void> write(AppLanguage language);
}

class SharedPreferencesAppLanguageStore implements AppLanguageStore {
  SharedPreferencesAppLanguageStore({SharedPreferencesAsync? preferences})
    : _preferences = preferences ?? SharedPreferencesAsync();

  static const key = 'app.language.v1';
  final SharedPreferencesAsync _preferences;

  @override
  Future<AppLanguage> read() async =>
      AppLanguage.fromTag(await _preferences.getString(key));

  @override
  Future<void> write(AppLanguage language) => switch (language.locale) {
    final locale? => _preferences.setString(key, locale.toLanguageTag()),
    null => _preferences.remove(key),
  };
}

class MemoryAppLanguageStore implements AppLanguageStore {
  MemoryAppLanguageStore([this.value = AppLanguage.system]);

  AppLanguage value;

  @override
  Future<AppLanguage> read() async => value;

  @override
  Future<void> write(AppLanguage language) async => value = language;
}

/// The OS per-app language that native menus, dialogs and notifications use.
///
/// [live] is true where the OS applies a change to the running app (Android
/// 13+); elsewhere (macOS) native strings switch on the next launch.
typedef AppLanguagePlatformState = ({AppLanguage language, bool live});

abstract interface class AppLanguagePlatform {
  /// The OS choice, or null when the OS has no per-app language.
  Future<AppLanguagePlatformState?> read();
  Future<void> apply(Locale? locale);
}

class MethodChannelAppLanguagePlatform implements AppLanguagePlatform {
  const MethodChannelAppLanguagePlatform();

  static const _channel = MethodChannel('com.copypaste.app/app_language');

  @override
  Future<AppLanguagePlatformState?> read() async {
    final result = await _invoke('getApplicationLocale', const {});
    if (result?['supported'] != true) return null;
    return (
      language: AppLanguage.fromTag(result?['tag'] as String?),
      live: result?['live'] != false,
    );
  }

  @override
  Future<void> apply(Locale? locale) =>
      _invoke('setApplicationLocale', {'tag': locale?.toLanguageTag()});

  Future<Map<Object?, Object?>?> _invoke(
    String method,
    Map<String, Object?> arguments,
  ) async {
    if (defaultTargetPlatform != TargetPlatform.android &&
        defaultTargetPlatform != TargetPlatform.macOS) {
      return null;
    }
    try {
      return await _channel.invokeMapMethod<Object?, Object?>(
        method,
        arguments,
      );
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    }
  }
}

/// Owns the interface language shared by every application window.
///
/// Where the OS keeps a per-app language it is the source of truth. When the
/// OS applies it live the app follows the system locales; otherwise the
/// choice is forced in Flutter and native strings follow after a relaunch.
class AppLanguageController extends ChangeNotifier with WidgetsBindingObserver {
  AppLanguageController({
    required AppLanguageStore store,
    AppLanguagePlatform? platform,
  }) : _store = store,
       _platform = platform;

  final AppLanguageStore _store;
  final AppLanguagePlatform? _platform;
  AppLanguage _language = AppLanguage.system;
  bool _followsPlatform = false;

  AppLanguage get language => _language;

  /// The locale to force, or null to resolve from the system locales.
  Locale? get locale => _followsPlatform ? null : _language.locale;

  Future<void> initialize() async {
    final platform = await _platform?.read();
    _followsPlatform = platform?.live ?? false;
    final next = platform?.language ?? await _store.read();
    if (_followsPlatform) WidgetsBinding.instance.addObserver(this);
    if (next == _language) return;
    _language = next;
    notifyListeners();
  }

  @override
  void didChangeLocales(List<Locale>? locales) {
    if (!_followsPlatform) return;
    _platform?.read().then((value) {
      if (value == null || value.language == _language) return;
      _language = value.language;
      notifyListeners();
    });
  }

  @override
  void dispose() {
    if (_followsPlatform) WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  Future<void> select(AppLanguage language) async {
    if (language == _language) return;
    _language = language;
    notifyListeners();
    await _store.write(language);
    await _platform?.apply(language.locale);
  }
}
