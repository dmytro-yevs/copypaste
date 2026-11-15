import 'package:shadcn_flutter/shadcn_flutter.dart';

/// A text controller that tints `{{ … }}` placeholders while [highlight] is
/// on. Placeholders stay plain text, so selection, undo and screen readers
/// treat them like the rest of the text.
class PlaceholderTextController extends TextEditingController {
  PlaceholderTextController({super.text, this.highlight = true});

  bool highlight;

  static final _placeholder = RegExp(r'(?<!\\)\{\{.*?\}\}', dotAll: true);

  @override
  TextSpan buildTextSpan({
    required BuildContext context,
    TextStyle? style,
    required bool withComposing,
  }) {
    if (!highlight || (withComposing && value.isComposingRangeValid)) {
      return super.buildTextSpan(
        context: context,
        style: style,
        withComposing: withComposing,
      );
    }
    final accent = style?.copyWith(
      color: Theme.of(context).colorScheme.primary,
      fontWeight: FontWeight.w600,
    );
    final spans = <TextSpan>[];
    var start = 0;
    for (final match in _placeholder.allMatches(text)) {
      if (match.start > start) {
        spans.add(TextSpan(text: text.substring(start, match.start)));
      }
      spans.add(TextSpan(text: match[0], style: accent));
      start = match.end;
    }
    if (start < text.length) spans.add(TextSpan(text: text.substring(start)));
    return TextSpan(style: style, children: spans);
  }
}
