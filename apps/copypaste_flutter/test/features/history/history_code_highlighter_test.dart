import 'package:copypaste_flutter/features/history/presentation/history_code_highlighter.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  test('detects a code language and returns themed spans', () {
    final result = HistoryCodeHighlighter.highlight(
      'fn main() { println!("hello"); }',
      const ThemeData(),
    );

    expect(result.language, 'Rust');
    expect(result.span.toPlainText(), contains('println'));
  });

  test('uses the explicit JSON grammar', () {
    final result = HistoryCodeHighlighter.highlight(
      '{"name":"CopyPaste"}',
      const ThemeData(),
      json: true,
    );

    expect(result.language, 'JSON');
    expect(result.span.toPlainText(), '{"name":"CopyPaste"}');
  });

  test('bounds highlighting work while keeping the complete code visible', () {
    final source = List.filled(20000, 'fn main() {}\n').join();
    final result = HistoryCodeHighlighter.highlight(source, const ThemeData());

    expect(result.language, 'Rust');
    expect(result.span.toPlainText(), source);
  });
}
