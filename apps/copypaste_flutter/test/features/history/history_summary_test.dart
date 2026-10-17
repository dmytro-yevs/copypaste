import 'dart:convert';

import 'package:copypaste_flutter/features/history/controller/history_summary.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:flutter_test/flutter_test.dart';

HistoryClip detail(String text) => HistoryClip(
  id: 'detail',
  contentType: 'text/plain',
  preview: text,
  body: text,
  createdAt: DateTime.utc(2026),
  pinned: true,
  secret: true,
);

void main() {
  test(
    'detail payload becomes a bounded list preview without losing identity',
    () {
      final full = detail('x' * (2 * 1024 * 1024));
      final summary = historySummary(full);
      expect(summary.body, isNull);
      expect(summary.preview, 'x' * 1024);
      expect(summary.truncated, isTrue);
      expect(summary.id, full.id);
      expect(summary.createdAt, full.createdAt);
      expect(summary.secret, isTrue);
      expect(summary.pinned, isTrue);
      expect(full.body!.length, 2 * 1024 * 1024);
    },
  );

  test('preview boundaries preserve complete multi-codepoint graphemes', () {
    const cluster = '👨‍👩‍👧‍👦';
    final summary = historySummary(detail(cluster * 100));
    expect(summary.preview, cluster * (1024 ~/ utf8.encode(cluster).length));
    expect(utf8.encode(summary.preview).length, lessThanOrEqualTo(1024));
    expect(summary.truncated, isTrue);
  });

  test('one oversized first grapheme follows the existing server contract', () {
    final cluster = 'a${'\u0301' * 600}';
    final summary = historySummary(detail('${cluster}tail'));
    expect(summary.preview, cluster);
    expect(summary.body, isNull);
    expect(summary.truncated, isTrue);
  });
}
