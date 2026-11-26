import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart';

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

/// Formats the weekday, day and month of a calendar date, without the year.
String formatSystemWeekdayDate(BuildContext context, DateTime value) {
  final localizations = Localizations.of<MaterialLocalizations>(
    context,
    MaterialLocalizations,
  );
  // Only the global Material delegate loads non-English date symbols.
  final locale = localizations is GlobalMaterialLocalizations
      ? Localizations.localeOf(context).toString()
      : 'en_US';
  return DateFormat.MMMMEEEEd(locale).format(value.toLocal());
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

/// Formats a timestamp to the second, for event logs.
String formatSystemTimestamp(BuildContext context, DateTime value) {
  final local = value.toLocal();
  final localizations = Localizations.of<MaterialLocalizations>(
    context,
    MaterialLocalizations,
  );
  final locale = localizations is GlobalMaterialLocalizations
      ? Localizations.localeOf(context).toString()
      : 'en_US';
  final time = MediaQuery.alwaysUse24HourFormatOf(context)
      ? DateFormat.Hms(locale)
      : DateFormat.jms(locale);
  return '${formatSystemDate(context, local)} ${time.format(local)}';
}

/// Recent activity uses the system clock format; older events retain the date.
String formatSystemRecentTime(BuildContext context, DateTime value) {
  final local = value.toLocal();
  final now = DateTime.now();
  final localizations = Localizations.of<MaterialLocalizations>(
    context,
    MaterialLocalizations,
  );
  if (localizations == null ||
      local.year != now.year ||
      local.month != now.month ||
      local.day != now.day) {
    return formatSystemDateTime(context, value);
  }
  return localizations.formatTimeOfDay(
    TimeOfDay.fromDateTime(local),
    alwaysUse24HourFormat: MediaQuery.alwaysUse24HourFormatOf(context),
  );
}

/// A period in the style of [formatSystemRecentTime]; an end on the day it
/// began shows only its time.
String formatSystemRecentPeriod(
  BuildContext context,
  DateTime start,
  DateTime end,
) {
  final from = start.toLocal();
  final to = end.toLocal();
  final localizations = Localizations.of<MaterialLocalizations>(
    context,
    MaterialLocalizations,
  );
  final sameDay =
      from.year == to.year && from.month == to.month && from.day == to.day;
  final ended = sameDay && localizations != null
      ? localizations.formatTimeOfDay(
          TimeOfDay.fromDateTime(to),
          alwaysUse24HourFormat: MediaQuery.alwaysUse24HourFormatOf(context),
        )
      : formatSystemRecentTime(context, end);
  return '${formatSystemRecentTime(context, start)} – $ended';
}

/// Compact, locale-aware inclusive calendar range with a single-date shortcut.
String formatSystemDateRange(
  BuildContext context,
  DateTime? start,
  DateTime? end,
) {
  final localizations = Localizations.of<MaterialLocalizations>(
    context,
    MaterialLocalizations,
  );
  String format(DateTime? value) {
    if (value == null) return '…';
    final local = value.toLocal();
    return localizations?.formatCompactDate(local) ??
        local.toIso8601String().split('T').first;
  }

  final from = start?.toLocal();
  final through = end?.toLocal();
  if (from != null &&
      through != null &&
      from.year == through.year &&
      from.month == through.month &&
      from.day == through.day) {
    return format(from);
  }
  return '${format(from)} – ${format(through)}';
}
