import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';

import 'app_localizations.dart';

export 'app_localizations.dart';

/// Localization settings shared by every application root.
abstract final class AppLocalization {
  static const fallback = Locale('en');

  static const delegates = <LocalizationsDelegate<dynamic>>[
    AppLocalizations.delegate,
    ...GlobalMaterialLocalizations.delegates,
  ];

  static const supportedLocales = AppLocalizations.supportedLocales;

  /// Picks the first preferred language the app ships, else English.
  static Locale resolve(List<Locale>? preferred, Iterable<Locale> supported) {
    for (final locale in preferred ?? const <Locale>[]) {
      for (final candidate in supported) {
        if (candidate.languageCode == locale.languageCode) return candidate;
      }
    }
    return fallback;
  }
}

/// Localized strings for code that has no [BuildContext], such as controllers.
///
/// [AppStringsLocale] keeps it on the resolved interface locale; tests and
/// widgets mounted without [AppLocalizations.delegate] read English.
abstract final class AppStrings {
  static Locale _locale = AppLocalization.fallback;
  static AppLocalizations _current = lookupAppLocalizations(_locale);

  static AppLocalizations get current => _current;

  static void useLocale(Locale locale) {
    final next = AppLocalizations.delegate.isSupported(locale)
        ? locale
        : AppLocalization.fallback;
    if (next == _locale) return;
    _locale = next;
    _current = lookupAppLocalizations(next);
  }
}

/// Moves [AppStrings] to the locale resolved for [child].
class AppStringsLocale extends StatelessWidget {
  const AppStringsLocale({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    AppStrings.useLocale(Localizations.localeOf(context));
    return child;
  }
}

extension AppLocalizationsContext on BuildContext {
  /// Strings for the nearest [Localizations], falling back to [AppStrings].
  AppLocalizations get l10n =>
      Localizations.of<AppLocalizations>(this, AppLocalizations) ??
      AppStrings.current;
}
