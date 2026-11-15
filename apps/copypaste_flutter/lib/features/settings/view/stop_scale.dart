import 'dart:async';

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';

/// A stock slider that moves between discrete stops and names its value.
///
/// The value is named through [header], beside the setting's copy; the track
/// follows with only its end stops named. A change is committed once, when the
/// pointer is released or a key is pressed; [onCommit] completes when the
/// change was applied, declined or failed, and the scale then shows [value]
/// again. A stop that is not confirmed therefore snaps back.
class StopScale extends StatefulWidget {
  const StopScale({
    super.key,
    required this.header,
    required this.label,
    required this.stops,
    required this.value,
    required this.stopLabel,
    required this.onCommit,
    this.pending = false,
  });

  /// Places the shown value, normally beside the setting's copy.
  final Widget Function(Widget value) header;

  /// The accessible name, normally the setting's title.
  final String label;
  final List<int> stops;

  /// The stored value; it must be one of [stops].
  final int value;
  final String Function(int value) stopLabel;

  /// Null when the value cannot be changed now.
  final Future<void> Function(int value)? onCommit;

  /// The value is being applied; other stops chosen meanwhile snap back.
  final bool pending;

  @override
  State<StopScale> createState() => _StopScaleState();
}

class _StopScaleState extends State<StopScale> {
  /// The stop under a dragged thumb; it only names the value.
  int? _dragIndex;

  /// A stop chosen while the pointer is still down, or a refused stop shown
  /// for one frame so the slider resynchronises its thumb.
  int? _candidateIndex;

  /// A committed stop waiting for [StopScale.onCommit].
  int? _proposedIndex;
  int _pointers = 0;
  bool _focused = false;

  int get _storedIndex => widget.stops.indexOf(widget.value);
  int get _sliderIndex => _candidateIndex ?? _proposedIndex ?? _storedIndex;
  int get _shownIndex => _dragIndex ?? _sliderIndex;
  bool get _enabled => widget.onCommit != null;
  bool get _accepting => _enabled && !widget.pending && _proposedIndex == null;

  void _changeEnded(int index) {
    _dragIndex = null;
    if (_pointers > 0) {
      setState(() => _candidateIndex = index);
    } else {
      unawaited(_commit(index));
    }
  }

  /// A cancelled pointer drops the stop it chose; a released one commits it.
  void _pointerEnded({required bool released}) {
    _pointers = (_pointers - 1).clamp(0, 1 << 20);
    // Gesture callbacks for this pointer run after the listener.
    scheduleMicrotask(() {
      if (!mounted || _pointers > 0) return;
      final candidate = _candidateIndex;
      _dragIndex = null;
      _candidateIndex = null;
      if (candidate == null || !released) {
        setState(() {});
      } else {
        unawaited(_commit(candidate));
      }
    });
  }

  Future<void> _commit(int index) async {
    final target = index.clamp(0, widget.stops.length - 1);
    if (!_accepting || target == _storedIndex) {
      setState(() => _candidateIndex = target);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) setState(() => _candidateIndex = null);
      });
      return;
    }
    setState(() {
      _candidateIndex = null;
      _proposedIndex = target;
    });
    try {
      await widget.onCommit!(widget.stops[target]);
    } finally {
      if (mounted) setState(() => _proposedIndex = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    final last = widget.stops.length - 1;
    final index = _shownIndex;
    final valueText = widget.stopLabel(widget.stops[index]);
    final slider = _TouchTarget(
      height: AppTheme.stopScaleHitHeight(),
      child: Shortcuts(
        shortcuts: const {
          SingleActivator(LogicalKeyboardKey.home): _FirstStopIntent(),
          SingleActivator(LogicalKeyboardKey.end): _LastStopIntent(),
        },
        child: Actions(
          actions: {
            _FirstStopIntent: CallbackAction<_FirstStopIntent>(
              onInvoke: (_) => _commit(0),
            ),
            _LastStopIntent: CallbackAction<_LastStopIntent>(
              onInvoke: (_) => _commit(last),
            ),
          },
          child: Focus(
            canRequestFocus: false,
            skipTraversal: true,
            includeSemantics: false,
            onFocusChange: (focused) => setState(() => _focused = focused),
            child: Listener(
              onPointerDown: (_) => _pointers++,
              onPointerUp: (_) => _pointerEnded(released: true),
              onPointerCancel: (_) => _pointerEnded(released: false),
              child: Slider(
                value: SliderValue.single(_sliderIndex.toDouble()),
                min: 0,
                max: last.toDouble(),
                divisions: last,
                enabled: _enabled,
                onChanged: (value) =>
                    setState(() => _dragIndex = value.value.round()),
                onChangeEnd: (value) => _changeEnded(value.value.round()),
              ),
            ),
          ),
        ),
      ),
    );
    // The slider announces the value, so the value text stays silent. As one
    // text it keeps its natural width beside the copy and wraps when stacked.
    final value = ExcludeSemantics(
      child: Text.rich(
        TextSpan(
          children: [
            if (widget.pending)
              const WidgetSpan(
                alignment: PlaceholderAlignment.middle,
                child: Padding(
                  padding: EdgeInsetsDirectional.only(end: AppSpacing.sm),
                  child: SizedBox.square(
                    dimension: AppIconSize.sm,
                    child: AppIconFrame(
                      size: AppIconSize.sm,
                      child: CircularProgressIndicator(),
                    ),
                  ),
                ),
              ),
            TextSpan(text: valueText),
          ],
        ),
        textAlign: TextAlign.end,
      ).small().medium(),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        widget.header(value),
        const Gap(AppSpacing.sm),
        Semantics(
          container: true,
          slider: true,
          label: widget.label,
          value: valueText,
          enabled: _enabled,
          focused: _focused,
          increasedValue: index < last
              ? widget.stopLabel(widget.stops[index + 1])
              : null,
          decreasedValue: index > 0
              ? widget.stopLabel(widget.stops[index - 1])
              : null,
          onIncrease: _accepting && index < last
              ? () => _commit(index + 1)
              : null,
          onDecrease: _accepting && index > 0 ? () => _commit(index - 1) : null,
          child: ExcludeSemantics(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                slider,
                if (last > 0)
                  _EndLabels(
                    first: widget.stopLabel(widget.stops.first),
                    last: widget.stopLabel(widget.stops.last),
                  ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class _FirstStopIntent extends Intent {
  const _FirstStopIntent();
}

class _LastStopIntent extends Intent {
  const _LastStopIntent();
}

/// The end stops' names under the track's ends; each shortens before it
/// overlaps the other.
class _EndLabels extends StatelessWidget {
  const _EndLabels({required this.first, required this.last});

  final String first;
  final String last;

  @override
  Widget build(BuildContext context) {
    final style = AppTheme.stopScaleLabelStyle(context);
    Widget label(String text, TextAlign align) => Flexible(
      child: Text(
        text,
        style: style,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        textAlign: align,
      ),
    );
    return Padding(
      padding: const EdgeInsets.only(top: AppSpacing.xs),
      // The stock slider lays its track out left to right, so the names
      // follow the same physical ends.
      child: Row(
        textDirection: TextDirection.ltr,
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          label(first, TextAlign.left),
          const Gap(AppSpacing.sm),
          label(last, TextAlign.right),
        ],
      ),
    );
  }
}

/// Centres a control in at least [height] and passes every touch inside that
/// band to it at the same horizontal position, on the control's centre line.
/// The band takes part in layout, so it never covers neighbours.
class _TouchTarget extends SingleChildRenderObjectWidget {
  const _TouchTarget({required this.height, required super.child});

  final double? height;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderTouchTarget(height ?? 0);

  @override
  void updateRenderObject(
    BuildContext context,
    _RenderTouchTarget renderObject,
  ) {
    renderObject.height = height ?? 0;
  }
}

class _RenderTouchTarget extends RenderShiftedBox {
  _RenderTouchTarget(this._height) : super(null);

  double _height;
  set height(double value) {
    if (value == _height) return;
    _height = value;
    markNeedsLayout();
  }

  @override
  void performLayout() {
    final child = this.child!;
    child.layout(constraints.copyWith(minHeight: 0), parentUsesSize: true);
    size = constraints.constrain(
      Size(
        child.size.width,
        child.size.height > _height ? child.size.height : _height,
      ),
    );
    (child.parentData! as BoxParentData).offset = Offset(
      0,
      (size.height - child.size.height) / 2,
    );
  }

  @override
  bool hitTestChildren(BoxHitTestResult result, {required Offset position}) {
    final child = this.child!;
    final offset = (child.parentData! as BoxParentData).offset;
    if (position.dx < 0 || position.dx >= size.width) return false;
    return result.addWithPaintOffset(
      offset: offset,
      position: Offset(position.dx, offset.dy + child.size.height / 2),
      hitTest: (result, transformed) =>
          child.hitTest(result, position: transformed),
    );
  }
}
