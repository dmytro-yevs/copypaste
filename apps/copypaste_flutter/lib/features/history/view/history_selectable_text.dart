import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/rendering.dart'
    show
        RenderAbstractViewport,
        SelectAllSelectionEvent,
        SelectedContent,
        SelectionEvent,
        SelectionEventType,
        SelectionPoint,
        SelectionResult,
        SelectionStatus;
import 'package:flutter/scheduler.dart';
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
  _FlatText? _large;
  bool _largeResolved = false;

  @override
  void didUpdateWidget(HistorySelectableText oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.text != widget.text || oldWidget.span != widget.span) {
      _selection = const TextSelection.collapsed(offset: 0);
      _large = null;
      _largeResolved = false;
    }
  }

  _FlatText? get _largeText {
    if (!_largeResolved) {
      _largeResolved = true;
      final text = widget.text;
      if (text != null) {
        _large = text.length > _largeTextCodeUnits
            ? _FlatText(text, [(start: 0, end: text.length, style: null)])
            : null;
      } else {
        final flat = _FlatText.from(widget.span!);
        _large = flat != null && flat.text.length > _largeTextCodeUnits
            ? flat
            : null;
      }
    }
    return _large;
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

  @override
  Widget build(BuildContext context) {
    final large = _largeText;
    if (large != null) {
      final config = _LargeTextConfig.resolve(
        context,
        style: widget.style ?? widget.span?.style,
        explicitStyle: widget.style,
      );
      if (config != null) {
        return _LargeSelectableText(
          key: ObjectKey(large),
          flat: large,
          config: config,
          onCopy: widget.onCopy,
        );
      }
    }
    void selectionChanged(
      TextSelection selection,
      SelectionChangedCause? cause,
    ) => _selection = selection;
    Widget menu(BuildContext context, EditableTextState editable) =>
        _selectionMenu(
          context,
          items: editable.contextMenuButtonItems,
          anchor: editable.contextMenuAnchors.primaryAnchor,
          copy: () async {
            if (await _copy() && editable.mounted) editable.hideToolbar();
          },
        );
    final child = widget.span == null
        ? SelectableText(
            widget.text!,
            style: widget.style,
            onSelectionChanged: selectionChanged,
            contextMenuBuilder: menu,
          )
        : SelectableText.rich(
            widget.span!,
            style: widget.style,
            onSelectionChanged: selectionChanged,
            contextMenuBuilder: menu,
          );
    return _copySemantics(copy: _copy, child: child);
  }
}

/// [label] replaces the merged child semantics where only some slices exist.
Widget _copySemantics({
  required Future<bool> Function() copy,
  required Widget child,
  String? label,
}) {
  return MergeSemantics(
    child: Semantics(
      label: label,
      excludeSemantics: label != null,
      onCopy: () => unawaited(copy()),
      child: Actions(
        actions: {
          CopySelectionTextIntent: CallbackAction<CopySelectionTextIntent>(
            onInvoke: (_) {
              unawaited(copy());
              return null;
            },
          ),
        },
        child: child,
      ),
    ),
  );
}

Widget _selectionMenu(
  BuildContext context, {
  required List<ContextMenuButtonItem> items,
  required Offset anchor,
  required Future<void> Function() copy,
}) {
  final labels = ShadcnLocalizations.of(context);
  // Reuse the library's popover placement and themed Button controls; only
  // the copy action differs from the stock read-only selection toolbar.
  return TextFieldTapRegion(
    child: ShadcnUI(
      child: PopoverOverlayWidget(
        anchor: ContextAnchor(context),
        position: anchor,
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
                  style: AppTheme.controlButtonStyle(const ButtonStyle.ghost()),
                  onPressed: item.type == ContextMenuButtonType.copy
                      ? copy
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

/// Engine layout costs about 300 bytes per character, so one paragraph for a
/// multi-megabyte clip retains hundreds of megabytes. Above this size only the
/// slices near the viewport are laid out.
const int _largeTextCodeUnits = 32 * 1024;
const int _sliceCodeUnits = 4 * 1024;
// shadcn's SelectableText lays text out in an EditableText, which reserves a
// 1 px caret gap plus the cursor width (2 px by default) at the line end.
const double _editableCaretGap = 1;
const double _defaultCursorWidth = 2;

typedef _Run = ({int start, int end, TextStyle? style});

/// Plain text plus the effective style of each source run, so any range can
/// be rebuilt as a span that paints exactly like that part of the original.
final class _FlatText {
  _FlatText(this.text, this.runs);

  final String text;
  final List<_Run> runs;

  static _FlatText? from(InlineSpan span) {
    final buffer = StringBuffer();
    final runs = <_Run>[];
    bool visit(InlineSpan span, TextStyle? inherited) {
      if (span is! TextSpan) return false;
      final own = span.style;
      final style = inherited == null || own == null
          ? own ?? inherited
          : inherited.merge(own);
      final text = span.text;
      if (text != null && text.isNotEmpty) {
        runs.add((
          start: buffer.length,
          end: buffer.length + text.length,
          style: style,
        ));
        buffer.write(text);
      }
      for (final child in span.children ?? const <InlineSpan>[]) {
        if (!visit(child, style)) return false;
      }
      return true;
    }

    return visit(span, null) ? _FlatText(buffer.toString(), runs) : null;
  }

  List<TextSpan> slice(int start, int end) {
    var low = 0;
    var high = runs.length;
    while (low < high) {
      final mid = (low + high) >> 1;
      if (runs[mid].end <= start) {
        low = mid + 1;
      } else {
        high = mid;
      }
    }
    final children = <TextSpan>[];
    for (var i = low; i < runs.length && runs[i].start < end; i++) {
      final run = runs[i];
      final from = math.max(start, run.start);
      final to = math.min(end, run.end);
      if (to > from) {
        children.add(
          TextSpan(text: text.substring(from, to), style: run.style),
        );
      }
    }
    return children.isEmpty ? const [TextSpan(text: '')] : children;
  }
}

/// The exact paragraph inputs shadcn's SelectableText would use here.
@immutable
final class _LargeTextConfig {
  const _LargeTextConfig({
    required this.style,
    required this.rootStyle,
    required this.strut,
    required this.align,
    required this.direction,
    required this.scaler,
    required this.locale,
    required this.widthBasis,
    required this.heightBehavior,
    required this.caretMargin,
    required this.selectionColor,
  });

  final TextStyle style;
  final TextStyle rootStyle;
  final StrutStyle strut;
  final TextAlign align;
  final TextDirection direction;
  final TextScaler scaler;
  final Locale? locale;
  final TextWidthBasis widthBasis;
  final TextHeightBehavior? heightBehavior;
  final double caretMargin;
  final Color selectionColor;

  /// Returns null where slicing could change the paragraph: outside a
  /// vertical scroll view, with a line limit, justified text, a height
  /// behavior that treats the first and last line differently, or platform
  /// spacing overrides that Text and EditableText apply differently.
  static _LargeTextConfig? resolve(
    BuildContext context, {
    required TextStyle? style,
    required TextStyle? explicitStyle,
  }) {
    final scrollable = Scrollable.maybeOf(context);
    if (scrollable == null || scrollable.position.axis != Axis.vertical) {
      return null;
    }
    final defaults = DefaultTextStyle.of(context);
    final heightBehavior =
        defaults.textHeightBehavior ??
        DefaultTextHeightBehavior.maybeOf(context);
    final align = defaults.textAlign ?? TextAlign.start;
    if (defaults.maxLines != null ||
        align == TextAlign.justify ||
        (heightBehavior != null &&
            (!heightBehavior.applyHeightToFirstAscent ||
                !heightBehavior.applyHeightToLastDescent)) ||
        MediaQuery.maybeLineHeightScaleFactorOverrideOf(context) != null ||
        MediaQuery.maybeLetterSpacingOverrideOf(context) != null ||
        MediaQuery.maybeWordSpacingOverrideOf(context) != null) {
      return null;
    }
    var effective = explicitStyle != null && !explicitStyle.inherit
        ? explicitStyle
        : defaults.style.merge(style);
    final strut = const StrutStyle().inheritFromTextStyle(effective);
    var root = defaults.style;
    if (MediaQuery.boldTextOf(context)) {
      const bold = TextStyle(fontWeight: FontWeight.bold);
      effective = effective.merge(bold);
      root = root.merge(bold);
    }
    final theme = Theme.of(context);
    final cursorWidth =
        ComponentTheme.maybeOf<SelectableTextTheme>(context)?.cursorWidth ??
        _defaultCursorWidth;
    return _LargeTextConfig(
      style: effective,
      rootStyle: root,
      strut: strut,
      align: align,
      direction: Directionality.of(context),
      scaler: MediaQuery.textScalerOf(context),
      locale: Localizations.maybeLocaleOf(context),
      widthBasis: defaults.textWidthBasis,
      heightBehavior: heightBehavior,
      caretMargin: _editableCaretGap + cursorWidth,
      selectionColor:
          DefaultSelectionStyle.of(context).selectionColor ??
          theme.colorScheme.primary.scaleAlpha(0.2),
    );
  }

  TextSpan span(_FlatText flat, int start, int end) => TextSpan(
    style: rootStyle,
    children: [TextSpan(style: style, children: flat.slice(start, end))],
  );

  @override
  bool operator ==(Object other) =>
      other is _LargeTextConfig &&
      other.style == style &&
      other.rootStyle == rootStyle &&
      other.strut == strut &&
      other.align == align &&
      other.direction == direction &&
      other.scaler == scaler &&
      other.locale == locale &&
      other.widthBasis == widthBasis &&
      other.heightBehavior == heightBehavior &&
      other.caretMargin == caretMargin &&
      other.selectionColor == selectionColor;

  @override
  int get hashCode => Object.hash(
    style,
    rootStyle,
    strut,
    align,
    direction,
    scaler,
    locale,
    widthBasis,
    heightBehavior,
    caretMargin,
    selectionColor,
  );
}

typedef _Slice = ({int start, int end, double top});

/// Slice boundaries fall on hard line breaks, or on soft-wrap line starts
/// inside a long line, so every slice wraps exactly like the full paragraph.
final class _SliceLayout {
  _SliceLayout(this.slices, this.width, this.height);

  final List<_Slice> slices;
  final double width;
  final double height;

  static _SliceLayout measure(
    _FlatText flat,
    _LargeTextConfig config,
    double maxWidth,
  ) {
    final text = flat.text;
    final painter = TextPainter(
      textAlign: config.align,
      textDirection: config.direction,
      textScaler: config.scaler,
      locale: config.locale,
      strutStyle: config.strut,
      textWidthBasis: config.widthBasis,
      textHeightBehavior: config.heightBehavior,
    );
    final slices = <_Slice>[];
    var start = 0;
    var top = 0.0;
    var width = 0.0;
    while (true) {
      var end = math.min(start + _sliceCodeUnits, text.length);
      var next = end;
      var wrapped = false;
      if (end < text.length) {
        final newline = text.lastIndexOf('\n', end - 1);
        if (newline >= start) {
          end = newline;
          next = newline + 1;
        } else {
          wrapped = true;
          if (end - start > 1 && _isHighSurrogate(text.codeUnitAt(end - 1))) {
            end -= 1;
          }
          next = end;
        }
      }
      painter
        ..text = config.span(flat, start, end)
        ..layout(maxWidth: maxWidth);
      var height = painter.height;
      width = math.max(width, painter.width);
      if (wrapped) {
        final lineStart = painter
            .getLineBoundary(TextPosition(offset: end - start - 1))
            .start;
        if (lineStart > 0) {
          height = painter
              .getOffsetForCaret(TextPosition(offset: lineStart), Rect.zero)
              .dy;
          end = start + lineStart;
          next = end;
        }
      }
      slices.add((start: start, end: end, top: top));
      top += height;
      if (end == text.length) break;
      start = next;
    }
    painter.dispose();
    return _SliceLayout(slices, width, top);
  }

  double bottomOf(int index) =>
      index + 1 < slices.length ? slices[index + 1].top : height;

  /// Indices of slices intersecting `[from, to)`.
  (int, int) range(double from, double to) {
    var low = 0;
    var high = slices.length;
    while (low < high) {
      final mid = (low + high) >> 1;
      if (bottomOf(mid) <= from) {
        low = mid + 1;
      } else {
        high = mid;
      }
    }
    final first = math.min(low, slices.length - 1);
    var last = first;
    while (last + 1 < slices.length && slices[last + 1].top < to) {
      last += 1;
    }
    return (first, last);
  }
}

bool _isHighSurrogate(int unit) => unit >= 0xD800 && unit <= 0xDBFF;

class _LargeSelectableText extends StatefulWidget {
  const _LargeSelectableText({
    super.key,
    required this.flat,
    required this.config,
    required this.onCopy,
  });

  final _FlatText flat;
  final _LargeTextConfig config;
  final Future<bool> Function(String text) onCopy;

  @override
  State<_LargeSelectableText> createState() => _LargeSelectableTextState();
}

class _LargeSelectableTextState extends State<_LargeSelectableText> {
  final GlobalKey _boxKey = GlobalKey();
  late final _LargeTextSelectionDelegate _delegate =
      _LargeTextSelectionDelegate(this);
  final Map<int, SelectionListenerNotifier> _notifiers = {};
  ({_LargeTextConfig config, double width, _SliceLayout layout})? _measured;
  ScrollPosition? _position;
  double _contentTop = 0;
  (int, int)? _built;
  Set<int> _retained = const {};
  bool _refreshScheduled = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final position = Scrollable.maybeOf(context)?.position;
    if (!identical(position, _position)) {
      _position?.removeListener(_handleScroll);
      _position = position?..addListener(_handleScroll);
    }
  }

  @override
  void dispose() {
    _position?.removeListener(_handleScroll);
    for (final notifier in _notifiers.values) {
      notifier.dispose();
    }
    _delegate.dispose();
    super.dispose();
  }

  _SliceLayout _layoutFor(double maxWidth) {
    final measured = _measured;
    if (measured != null &&
        measured.config == widget.config &&
        measured.width == maxWidth) {
      return measured.layout;
    }
    final layout = _SliceLayout.measure(widget.flat, widget.config, maxWidth);
    _measured = (config: widget.config, width: maxWidth, layout: layout);
    return layout;
  }

  (int, int) _visible(_SliceLayout layout) {
    final position = _position;
    final viewport = position != null && position.hasViewportDimension
        ? position.viewportDimension
        : MediaQuery.sizeOf(context).height;
    final top =
        (position != null && position.hasPixels ? position.pixels : 0) -
        _contentTop;
    return layout.range(top - viewport, top + viewport * 2);
  }

  void _handleScroll() {
    final layout = _measured?.layout;
    if (layout == null || _visible(layout) == _built) return;
    _refresh();
  }

  /// Rebuilds outside build and layout; scroll and selection callbacks can
  /// arrive in either phase.
  void _refresh() {
    if (!mounted) return;
    if (SchedulerBinding.instance.schedulerPhase !=
        SchedulerPhase.persistentCallbacks) {
      setState(() {});
      return;
    }
    if (_refreshScheduled) return;
    _refreshScheduled = true;
    SchedulerBinding.instance.addPostFrameCallback((_) {
      _refreshScheduled = false;
      if (mounted) setState(() {});
    });
  }

  void _afterBuild(Set<int> built) {
    SchedulerBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _notifiers.removeWhere((start, notifier) {
        if (built.contains(start)) return false;
        notifier.dispose();
        return true;
      });
      final box = _boxKey.currentContext?.findRenderObject();
      if (box is! RenderBox || !box.attached) return;
      final top = RenderAbstractViewport.maybeOf(
        box,
      )?.getOffsetToReveal(box, 0).offset;
      if (top != null && top != _contentTop) {
        _contentTop = top;
        _handleScroll();
      }
    });
  }

  SelectionListenerNotifier _notifierFor(int start) =>
      _notifiers.putIfAbsent(start, () {
        return SelectionListenerNotifier()..addListener(_handleSelection);
      });

  /// Selected slices stay laid out so a selection that scrolls out of view
  /// keeps its anchor. Select-all reselects slices as they are built instead.
  void _handleSelection() {
    final selected = _delegate.selectingAll
        ? const <int>{}
        : {
            for (final MapEntry(:key, :value) in _notifiers.entries)
              if (value.registered &&
                  value.selection.status != SelectionStatus.none)
                key,
          };
    if (selected.length == _retained.length &&
        selected.containsAll(_retained)) {
      return;
    }
    _retained = selected;
    _refresh();
  }

  String? _selectedText() {
    final text = widget.flat.text;
    if (_delegate.selectingAll) return text;
    int? from;
    int? to;
    for (final entry in _notifiers.entries) {
      final notifier = entry.value;
      if (!notifier.registered) continue;
      final details = notifier.selection;
      final range = details.range;
      if (details.status != SelectionStatus.uncollapsed || range == null) {
        continue;
      }
      final int low =
          entry.key + math.min<int>(range.startOffset, range.endOffset);
      final int high =
          entry.key + math.max<int>(range.startOffset, range.endOffset);
      from = from == null || low < from ? low : from;
      to = to == null || high > to ? high : to;
    }
    if (from == null || to == null || to <= from) return null;
    return text.substring(from, math.min(to, text.length));
  }

  Future<bool> _copy() async {
    final text = _selectedText();
    return text != null && text.isNotEmpty && await widget.onCopy(text);
  }

  void _reveal(SelectionPoint? point) {
    final box = _boxKey.currentContext?.findRenderObject();
    if (point == null || box is! RenderBox || !box.attached) return;
    final position = point.localPosition;
    box.showOnScreen(
      rect: Rect.fromLTWH(
        position.dx,
        position.dy - point.lineHeight,
        1,
        point.lineHeight,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final config = widget.config;
    return _copySemantics(
      copy: _copy,
      label: widget.flat.text,
      child: SelectableRegion(
        selectionControls: shadcnTextSelectionHandleControls,
        magnifierConfiguration: const TextMagnifierConfiguration(),
        contextMenuBuilder: (context, region) => _selectionMenu(
          context,
          items: region.contextMenuButtonItems,
          anchor: region.contextMenuAnchors.primaryAnchor,
          copy: () async {
            if (await _copy() && region.mounted) region.hideToolbar();
          },
        ),
        child: DefaultSelectionStyle.merge(
          selectionColor: config.selectionColor,
          child: LayoutBuilder(
            builder: (context, constraints) {
              final layoutWidth = math.max(
                0.0,
                constraints.maxWidth - config.caretMargin,
              );
              final layout = _layoutFor(layoutWidth);
              final visible = _visible(layout);
              _built = visible;
              final indices = <int>{
                for (var i = visible.$1; i <= visible.$2; i++) i,
              };
              if (_retained.isNotEmpty) {
                for (var i = 0; i < layout.slices.length; i++) {
                  if (_retained.contains(layout.slices[i].start)) {
                    indices.add(i);
                  }
                }
              }
              final built = <int>{};
              final children = <Widget>[];
              for (final index in indices.toList()..sort()) {
                final slice = layout.slices[index];
                built.add(slice.start);
                children.add(
                  Positioned(
                    key: ValueKey<int>(slice.start),
                    top: slice.top,
                    left: 0,
                    width: layoutWidth,
                    child: SelectionListener(
                      selectionNotifier: _notifierFor(slice.start),
                      child: Text.rich(
                        config.span(widget.flat, slice.start, slice.end),
                        textAlign: config.align,
                        textScaler: config.scaler,
                        locale: config.locale,
                        strutStyle: config.strut,
                        textWidthBasis: config.widthBasis,
                        textHeightBehavior: config.heightBehavior,
                        softWrap: true,
                        overflow: TextOverflow.clip,
                      ),
                    ),
                  ),
                );
              }
              _afterBuild(built);
              return SelectionContainer(
                delegate: _delegate,
                child: RepaintBoundary(
                  child: SizedBox(
                    key: _boxKey,
                    width: math.min(
                      constraints.maxWidth,
                      layout.width + config.caretMargin,
                    ),
                    height: layout.height,
                    child: Stack(clipBehavior: Clip.none, children: children),
                  ),
                ),
              );
            },
          ),
        ),
      ),
    );
  }
}

/// Native selection works on the slices that are laid out. This delegate
/// supplies what slicing would otherwise lose: select-all covers the whole
/// clip, copy reads the full selected range, and the selection edge is
/// revealed in the enclosing scroll view as EditableText does.
final class _LargeTextSelectionDelegate
    extends StaticSelectionContainerDelegate {
  _LargeTextSelectionDelegate(this._owner);

  final _LargeSelectableTextState _owner;
  bool selectingAll = false;

  @override
  SelectionResult dispatchSelectionEvent(SelectionEvent event) {
    selectingAll = event.type == SelectionEventType.selectAll;
    final result = super.dispatchSelectionEvent(event);
    switch (event.type) {
      case SelectionEventType.startEdgeUpdate:
        _owner._reveal(value.startSelectionPoint);
      case SelectionEventType.endEdgeUpdate:
      case SelectionEventType.granularlyExtendSelection:
      case SelectionEventType.directionallyExtendSelection:
        _owner._reveal(value.endSelectionPoint);
      case SelectionEventType.clear:
      case SelectionEventType.selectAll:
      case SelectionEventType.selectWord:
      case SelectionEventType.selectParagraph:
        break;
    }
    return result;
  }

  @override
  void didChangeSelectables() {
    super.didChangeSelectables();
    if (selectingAll) {
      super.dispatchSelectionEvent(const SelectAllSelectionEvent());
    }
  }

  @override
  SelectedContent? getSelectedContent() {
    final text = _owner._selectedText();
    return text == null ? null : SelectedContent(plainText: text);
  }
}
