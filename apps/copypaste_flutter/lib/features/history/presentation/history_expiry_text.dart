import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

/// "Deletes in N min" for an expiring clip. The text is rounded up to whole
/// minutes, hours or days and refreshes only when that value changes, with
/// one timer while it is shown. It is not a live region, so assistive
/// technology reads it on focus instead of on every change.
class HistoryExpiryText extends StatefulWidget {
  const HistoryExpiryText({
    super.key,
    required this.expiresAt,
    this.style,
    this.strutStyle,
    this.now = DateTime.now,
  });

  final DateTime expiresAt;
  final TextStyle? style;
  final StrutStyle? strutStyle;
  final DateTime Function() now;

  /// The countdown text for [remaining] and the time until it changes.
  static (String, Duration?) describe(
    AppLocalizations strings,
    Duration remaining,
  ) {
    if (remaining <= Duration.zero) return (strings.historyDeletesSoon, null);
    const minute = Duration(minutes: 1);
    const hour = Duration(hours: 1);
    const day = Duration(days: 1);
    final unit = remaining <= hour
        ? minute
        : remaining <= day * 2
        ? hour
        : day;
    final count =
        (remaining.inMicroseconds + unit.inMicroseconds - 1) ~/
        unit.inMicroseconds;
    final text = remaining < minute
        ? strings.historyDeletesSoon
        : switch (unit) {
            minute => strings.historyDeletesInMinutes(count),
            hour => strings.historyDeletesInHours(count),
            _ => strings.historyDeletesInDays(count),
          };
    final change = remaining < minute
        ? remaining
        : remaining - unit * (count - 1);
    return (text, change);
  }

  @override
  State<HistoryExpiryText> createState() => _HistoryExpiryTextState();
}

class _HistoryExpiryTextState extends State<HistoryExpiryText> {
  Timer? _timer;

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final (text, change) = HistoryExpiryText.describe(
      context.l10n,
      widget.expiresAt.difference(widget.now()),
    );
    _timer?.cancel();
    _timer = change == null
        ? null
        : Timer(change + const Duration(milliseconds: 1), () {
            if (mounted) setState(() {});
          });
    return Text(
      text,
      key: const ValueKey<String>('history-expiry-text'),
      style: widget.style,
      strutStyle: widget.strutStyle,
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
    );
  }
}
