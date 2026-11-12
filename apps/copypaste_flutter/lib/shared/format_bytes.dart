import 'package:intl/intl.dart';

import '../l10n/l10n.dart';

const int _kibibyte = 1024;
const int _mebibyte = 1024 * _kibibyte;
const int _gibibyte = 1024 * _mebibyte;

/// Shared size presentation: binary units labelled KiB, MiB and GiB, with
/// one decimal in the interface language's number format.
String formatBytes(int bytes) {
  final strings = AppStrings.current;
  if (bytes < _kibibyte) return strings.sharedSizeBytes('$bytes');
  if (bytes < _mebibyte) {
    return strings.sharedSizeKibibytes(formatSizeNumber(bytes / _kibibyte));
  }
  if (bytes < _gibibyte) {
    return strings.sharedSizeMebibytes(formatSizeNumber(bytes / _mebibyte));
  }
  return strings.sharedSizeGibibytes(formatSizeNumber(bytes / _gibibyte));
}

/// A size amount with one decimal, for messages that name the unit.
String formatSizeNumber(double value) =>
    NumberFormat('0.0', AppStrings.current.localeName).format(value);
