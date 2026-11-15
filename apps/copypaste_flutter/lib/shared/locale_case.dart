import 'dart:ui';

/// Upper-cases [value] by the rules of [locale]. Scripts without case are
/// returned unchanged.
String toLocaleUpperCase(String value, Locale locale) {
  // Turkic alphabets pair the dotted lowercase i with a dotted capital.
  final dotted = locale.languageCode == 'tr' || locale.languageCode == 'az';
  return (dotted ? value.replaceAll('i', 'İ') : value).toUpperCase();
}
