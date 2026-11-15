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

  test('memo reuses matching output and replaces changed inputs', () {
    final memo = HistoryCodeHighlightMemo();
    const light = ThemeData();
    const dark = ThemeData(colorScheme: ColorSchemes.darkSlate);
    const source = '{"name":"CopyPaste"}';
    final first = memo.highlight(source, light, json: true);
    expect(identical(memo.highlight(source, light, json: true), first), isTrue);
    expect(first.span.toPlainText(), source);
    expect(first.language, 'JSON');

    final otherSource = memo.highlight('{"name":"Other"}', light, json: true);
    expect(identical(otherSource, first), isFalse);
    expect(otherSource.span.toPlainText(), '{"name":"Other"}');
    final otherKind = memo.highlight(source, light);
    expect(identical(otherKind, first), isFalse);
    final jsonAgain = memo.highlight(source, light, json: true);
    final otherTheme = memo.highlight(source, dark, json: true);
    expect(identical(otherTheme, jsonAgain), isFalse);
    expect(otherTheme.span, isNot(equals(jsonAgain.span)));
    expect(otherTheme.span.toPlainText(), source);

    memo.clear();
    expect(
      identical(memo.highlight(source, dark, json: true), otherTheme),
      isFalse,
    );
  });

  test('memo does not retain oversized source or prior result', () {
    final memo = HistoryCodeHighlightMemo();
    const theme = ThemeData();
    const small = 'fn main() {}';
    final first = memo.highlight(small, theme);
    final large = List.filled(200001, 'a').join();
    final oversized = memo.highlight(large, theme, json: true);
    expect(oversized.span.toPlainText(), large);
    expect(
      identical(memo.highlight(large, theme, json: true), oversized),
      isFalse,
    );
    expect(identical(memo.highlight(small, theme), first), isFalse);
  });
}
