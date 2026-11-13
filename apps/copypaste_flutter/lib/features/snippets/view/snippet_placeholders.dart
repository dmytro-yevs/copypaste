import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../models/snippet_models.dart';

/// A body controller that tints `{{ … }}` placeholders while [highlight] is
/// on. Placeholders stay plain text, so selection, undo and screen readers
/// treat them like the rest of the body.
class PlaceholderTextController extends TextEditingController {
  PlaceholderTextController({super.text});

  bool highlight = false;

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

  /// Replaces the selection with [insert] and puts the caret after it.
  void insertAtSelection(String insert) {
    final selection = value.selection;
    final start = selection.isValid ? selection.start : text.length;
    final end = selection.isValid ? selection.end : text.length;
    value = TextEditingValue(
      text: text.replaceRange(start, end, insert),
      selection: TextSelection.collapsed(offset: start + insert.length),
    );
  }
}

/// A double-quoted placeholder value (spec T-4).
String quotePlaceholderValue(String value) =>
    '"${value.replaceAll(r'\', r'\\').replaceAll('"', r'\"')}"';

enum _PlaceholderKind {
  date,
  dateShort,
  dateLong,
  dateIso,
  dateNextWeek,
  time,
  datetime,
  clipboard,
  clipboardPrevious,
  field,
  cursor,
  uuid,
  random,
  upper,
  lower,
  trim,
}

/// The "Insert placeholder" palette. Resolves to the text to insert, or
/// null when dismissed. [abbreviations] are snippets that can be nested.
Future<String?> showPlaceholderPalette(
  BuildContext context, {
  required List<String> abbreviations,
}) => AppOverlays.showDialog<String>(
  context,
  builder: (context) =>
      _PlaceholderPalette(abbreviations: abbreviations.toSet().toList()),
);

class _PlaceholderPalette extends StatelessWidget {
  const _PlaceholderPalette({required this.abbreviations});

  final List<String> abbreviations;

  static String _label(AppLocalizations l10n, _PlaceholderKind kind) =>
      l10n.snippetsPlaceholderKind(kind.name);

  static String? _text(_PlaceholderKind kind) => switch (kind) {
    _PlaceholderKind.date => '{{date}}',
    _PlaceholderKind.dateShort => '{{date style=short}}',
    _PlaceholderKind.dateLong => '{{date style=long}}',
    _PlaceholderKind.dateIso => '{{date style=iso}}',
    _PlaceholderKind.dateNextWeek => '{{date offset=+1w}}',
    _PlaceholderKind.time => '{{time}}',
    _PlaceholderKind.datetime => '{{datetime}}',
    _PlaceholderKind.clipboard => '{{clipboard}}',
    _PlaceholderKind.clipboardPrevious => '{{clipboard index=1}}',
    _PlaceholderKind.field => null,
    _PlaceholderKind.cursor => '{{cursor}}',
    _PlaceholderKind.uuid => '{{uuid}}',
    _PlaceholderKind.random => '{{random min=1 max=100}}',
    _PlaceholderKind.upper => ' | upper',
    _PlaceholderKind.lower => ' | lower',
    _PlaceholderKind.trim => ' | trim',
  };

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    Future<void> choose(_PlaceholderKind kind) async {
      final text = _text(kind) ?? await showFieldPlaceholderBuilder(context);
      if (text != null && context.mounted) Navigator.pop(context, text);
    }

    bool matches(String label, String query) =>
        label.toLowerCase().contains(query.trim().toLowerCase());

    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): () =>
            Navigator.maybePop(context),
      },
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxHeight:
              MediaQuery.sizeOf(context).height *
              AppOverlaySize.dialogContentHeightFactor,
        ),
        child: AppTheme.commandSurface(
          context,
          child: Command(
            key: const ValueKey('snippet-placeholder-palette'),
            debounceDuration: Duration.zero,
            searchPlaceholder: Text(l10n.snippetsInsertPlaceholder),
            emptyBuilder: (context) => StateView.empty(
              compact: true,
              icon: LucideIcons.searchX,
              title: l10n.snippetsPlaceholderNoMatch,
            ),
            builder: (context, query) {
              final query_ = query ?? '';
              final items = [
                for (final kind in _PlaceholderKind.values)
                  if (matches(_label(l10n, kind), query_))
                    CommandItem(
                      key: ValueKey('snippet-placeholder-${kind.name}'),
                      leading: const AppIconFrame(
                        child: Icon(LucideIcons.braces),
                      ),
                      title: Text(_label(l10n, kind)),
                      trailing: Text(_text(kind) ?? '').muted().small(),
                      onTap: () => choose(kind),
                    ),
              ];
              final nested = [
                for (final abbreviation in abbreviations)
                  if (matches(abbreviation, query_))
                    CommandItem(
                      leading: const AppIconFrame(
                        child: Icon(LucideIcons.notebookText),
                      ),
                      title: Text(abbreviation),
                      onTap: () => Navigator.pop(
                        context,
                        '{{snippet abbr=${quotePlaceholderValue(abbreviation)}}}',
                      ),
                    ),
              ];
              if (items.isEmpty && nested.isEmpty) {
                return Stream.value(const []);
              }
              return Stream.value([
                if (items.isNotEmpty)
                  CommandCategory(
                    title: Text(l10n.snippetsPlaceholderGroup),
                    children: items,
                  ),
                if (nested.isNotEmpty)
                  CommandCategory(
                    title: Text(l10n.snippetsPlaceholderNestedGroup),
                    children: nested,
                  ),
              ]);
            },
          ),
        ),
      ),
    );
  }
}

/// Builds a `field` placeholder from its name, type, default and options.
Future<String?> showFieldPlaceholderBuilder(BuildContext context) =>
    AppOverlays.showDialog<String>(
      context,
      builder: (context) => const _FieldBuilder(),
    );

class _FieldBuilder extends StatefulWidget {
  const _FieldBuilder();

  @override
  State<_FieldBuilder> createState() => _FieldBuilderState();
}

class _FieldBuilderState extends State<_FieldBuilder> {
  String _name = '';
  TemplateFieldKind _kind = TemplateFieldKind.text;
  String _default = '';
  String _options = '';

  static const _maxName = 40;

  List<String> get _optionList => [
    for (final option in _options.split(','))
      if (option.trim().isNotEmpty) option.trim(),
  ];

  bool get _valid =>
      _name.trim().isNotEmpty &&
      _name.trim().length <= _maxName &&
      (_kind != TemplateFieldKind.choice || _optionList.isNotEmpty);

  String _build() {
    final parts = ['field', 'name=${quotePlaceholderValue(_name.trim())}'];
    if (_kind != TemplateFieldKind.text) parts.add('type=${_kind.name}');
    if (_kind == TemplateFieldKind.choice) {
      final options = _optionList
          .map((option) => option.replaceAll(',', r'\,'))
          .join(',');
      parts.add('options=${quotePlaceholderValue(options)}');
    }
    if (_default.isNotEmpty) {
      parts.add('default=${quotePlaceholderValue(_default)}');
    }
    return '{{${parts.join(' ')}}}';
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    Widget label(String text) => Padding(
      padding: const EdgeInsets.only(bottom: AppSpacing.xs),
      child: Text(text).small(),
    );
    return AppOverlays.alertDialog(
      icon: LucideIcons.textCursorInput,
      title: Text(l10n.snippetsFieldBuilderTitle),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          label(l10n.snippetsFieldName),
          TextField(
            key: const ValueKey('field-builder-name'),
            maxLength: _maxName,
            decoration: AppOverlays.dialogFieldDecoration(context),
            onChanged: (value) => setState(() => _name = value),
          ),
          const Gap(AppSpacing.md),
          label(l10n.snippetsFieldType),
          Select<TemplateFieldKind>(
            value: _kind,
            onChanged: (value) {
              if (value != null) setState(() => _kind = value);
            },
            itemBuilder: (context, kind) =>
                Text(l10n.snippetsFieldTypeName(kind.name)),
            popup: SelectPopup<TemplateFieldKind>(
              items: SelectItemList(
                children: [
                  for (final kind in TemplateFieldKind.values)
                    SelectItemButton<TemplateFieldKind>(
                      value: kind,
                      child: Text(l10n.snippetsFieldTypeName(kind.name)),
                    ),
                ],
              ),
            ).call,
          ),
          if (_kind == TemplateFieldKind.choice) ...[
            const Gap(AppSpacing.md),
            label(l10n.snippetsFieldOptions),
            TextField(
              decoration: AppOverlays.dialogFieldDecoration(context),
              onChanged: (value) => setState(() => _options = value),
            ),
          ],
          const Gap(AppSpacing.md),
          label(l10n.snippetsFieldDefault),
          TextField(
            decoration: AppOverlays.dialogFieldDecoration(context),
            onChanged: (value) => setState(() => _default = value),
          ),
        ],
      ),
      actions: [
        Button.ghost(
          alignment: Alignment.center,
          style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
          onPressed: () => Navigator.pop(context),
          child: Text(l10n.commonCancel),
        ),
        Button.primary(
          key: const ValueKey('field-builder-insert'),
          alignment: Alignment.center,
          style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
          onPressed: _valid ? () => Navigator.pop(context, _build()) : null,
          child: Text(l10n.snippetsInsert),
        ),
      ],
    );
  }
}
