import 'package:flutter/material.dart';

/// Formats a calendar date with the active system locale.
String formatSystemDate(BuildContext context, DateTime value) {
  final local = value.toLocal();
  final localizations = Localizations.of<MaterialLocalizations>(
    context,
    MaterialLocalizations,
  );
  return localizations?.formatShortDate(local) ??
      local.toIso8601String().split('T').first;
}

/// Formats application timestamps with the active system locale and clock.
String formatSystemDateTime(BuildContext context, DateTime value) {
  final local = value.toLocal();
  final localizations = Localizations.of<MaterialLocalizations>(
    context,
    MaterialLocalizations,
  );
  if (localizations == null) {
    return local.toString();
  }
  final date = localizations.formatShortDate(local);
  final time = localizations.formatTimeOfDay(
    TimeOfDay.fromDateTime(local),
    alwaysUse24HourFormat: MediaQuery.alwaysUse24HourFormatOf(context),
  );
  return '$date $time';
}
