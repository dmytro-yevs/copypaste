import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';

/// Keeps selection copy, keyboard copy and accessibility copy tied to a clip.
/// shadcn's stock selection menu does not expose a copy callback.
class HistorySelectableText extends StatefulWidget {
  const HistorySelectableText(
    this.text, {
    super.key,
    required this.onCopy,
    this.style,
  }) : span = null;
  const HistorySelectableText.rich(
    this.span, {
    super.key,
    required this.onCopy,
    this.style,
  }) : text = null;

  final String? text;
  final TextSpan? span;
  final TextStyle? style;
  final Future<bool> Function(String text) onCopy;

  @override
  State<HistorySelectableText> createState() => _HistorySelectableTextState();
}

class _HistorySelectableTextState extends State<HistorySelectableText> {
  TextSelection _selection = const TextSelection.collapsed(offset: 0);

  @override
  void didUpdateWidget(HistorySelectableText oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.text != widget.text || oldWidget.span != widget.span) {
      _selection = const TextSelection.collapsed(offset: 0);
    }
  }

  Future<bool> _copy() async {
    final text =
        widget.text ?? widget.span!.toPlainText(includeSemanticsLabels: false);
    if (!_selection.isValid ||
        _selection.isCollapsed ||
        _selection.end > text.length) {
      return false;
    }
    return widget.onCopy(_selection.textInside(text));
  }

  Widget _menu(BuildContext context, EditableTextState editable) {
    final labels = ShadcnLocalizations.of(context);
    final items = editable.contextMenuButtonItems;
    // Reuse the library's popover placement and themed Button controls; only
    // the copy action differs from the stock read-only selection toolbar.
    return TextFieldTapRegion(
      child: ShadcnUI(
        child: PopoverOverlayWidget(
          anchor: ContextAnchor(context),
          position: editable.contextMenuAnchors.primaryAnchor,
          anchorSize: Size.zero,
          alignment: Alignment.topLeft,
          anchorAlignment: Alignment.topLeft,
          animation: const AlwaysStoppedAnimation(1),
          follow: false,
          builder: (context) => Card(
            child: Wrap(
              children: [
                for (final item in items)
                  Button.ghost(
                    style: AppTheme.controlButtonStyle(
                      const ButtonStyle.ghost(),
                    ),
                    onPressed: item.type == ContextMenuButtonType.copy
                        ? () async {
                            if (await _copy() && editable.mounted) {
                              editable.hideToolbar();
                            }
                          }
                        : item.onPressed,
                    child: Text(
                      item.label ??
                          switch (item.type) {
                            ContextMenuButtonType.copy => labels.menuCopy,
                            ContextMenuButtonType.selectAll =>
                              labels.menuSelectAll,
                            ContextMenuButtonType.share => labels.menuShare,
                            ContextMenuButtonType.searchWeb =>
                              labels.menuSearchWeb,
                            _ => item.type.name,
                          },
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    void selectionChanged(
      TextSelection selection,
      SelectionChangedCause? cause,
    ) => _selection = selection;
    final child = widget.span == null
        ? SelectableText(
            widget.text!,
            style: widget.style,
            onSelectionChanged: selectionChanged,
            contextMenuBuilder: _menu,
          )
        : SelectableText.rich(
            widget.span!,
            style: widget.style,
            onSelectionChanged: selectionChanged,
            contextMenuBuilder: _menu,
          );
    return MergeSemantics(
      child: Semantics(
        onCopy: () => unawaited(_copy()),
        child: Actions(
          actions: {
            CopySelectionTextIntent: CallbackAction<CopySelectionTextIntent>(
              onInvoke: (_) {
                unawaited(_copy());
                return null;
              },
            ),
          },
          child: child,
        ),
      ),
    );
  }
}
