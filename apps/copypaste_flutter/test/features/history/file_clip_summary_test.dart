import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/file_clip_summary.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('a multi-file clip shows its count and first member names', (
    tester,
  ) async {
    final clip = HistoryClip(
      id: 'set',
      contentType: 'file',
      preview:
          '/docs/a.pdf\n/docs/b.png\n/docs/c.txt\n/docs/d.txt\n/docs/e.txt',
      createdAt: DateTime.utc(2026),
      pinned: false,
      file: HistoryFileDetails(
        name: 'a.pdf',
        sizeBytes: 50,
        fileCount: 5,
        files: [
          for (final name in ['a.pdf', 'b.png', 'c.txt', 'd.txt', 'e.txt'])
            HistoryFileMember(name: name, sizeBytes: 10),
        ],
      ),
    );
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: FileClipSummary(clip: clip)),
      ),
    );

    expect(find.text('5 files'), findsOneWidget);
    expect(find.text('a.pdf, b.png, c.txt'), findsOneWidget);
    expect(find.text(', +2'), findsOneWidget);
  });

  testWidgets('a single file shows its name once, over its folder', (
    tester,
  ) async {
    final clip = HistoryClip(
      id: 'file',
      contentType: 'file',
      preview: '/docs/reports/a.pdf',
      createdAt: DateTime.utc(2026),
      pinned: false,
      file: const HistoryFileDetails(
        name: 'a.pdf',
        sourceReference: '/docs/reports/a.pdf',
        sizeBytes: 10,
      ),
    );
    await tester.pumpWidget(
      ShadcnApp(
        home: Scaffold(child: FileClipSummary(clip: clip)),
      ),
    );

    expect(find.text('a.pdf'), findsOneWidget);
    expect(find.text('/docs/reports'), findsOneWidget);
  });
}
