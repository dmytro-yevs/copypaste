import 'dart:async';

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';

/// A stock slider that moves between discrete stops and names its value.
///
/// A change is committed once, when the pointer is released or a key is
/// pressed; [onCommit] completes when the change was applied, declined or
/// failed, and the scale then shows [value] again. A stop that is not
/// confirmed therefore snaps back.
class StopScale extends StatefulWidget {
  const StopScale({
    super.key,
    required this.label,
    required this.stops,
    required this.value,
    required this.stopLabel,
    required this.onCommit,
    this.pending = false,
  });

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
    return Semantics(
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
      onIncrease: _accepting && index < last ? () => _commit(index + 1) : null,
      onDecrease: _accepting && index > 0 ? () => _commit(index - 1) : null,
      child: ExcludeSemantics(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                if (widget.pending) ...[
                  const SizedBox.square(
                    dimension: AppIconSize.sm,
                    child: AppIconFrame(
                      size: AppIconSize.sm,
                      child: CircularProgressIndicator(),
                    ),
                  ),
                  const Gap(AppSpacing.sm),
                ],
                Flexible(child: Text(valueText).small().medium()),
              ],
            ),
            const Gap(AppSpacing.sm),
            slider,
            _StopLabels(
              labels: [for (final stop in widget.stops) widget.stopLabel(stop)],
            ),
          ],
        ),
      ),
    );
  }
}

class _FirstStopIntent extends Intent {
  const _FirstStopIntent();
}

class _LastStopIntent extends Intent {
  const _LastStopIntent();
}

/// Stop names under the track, at the stops' positions. All names show when
/// they fit, otherwise only the two ends, otherwise none; the current value is
/// always named above the track.
class _StopLabels extends StatelessWidget {
  const _StopLabels({required this.labels});

  final List<String> labels;

  @override
  Widget build(BuildContext context) {
    final style = AppTheme.stopScaleLabelStyle(context);
    final scaler = MediaQuery.textScalerOf(context);
    final direction = Directionality.of(context);
    final theme = Theme.of(context);
    final inset = theme.density.baseGap * theme.scaling / 2;
    return LayoutBuilder(
      builder: (context, constraints) {
        final width = constraints.maxWidth;
        final painters = [
          for (final label in labels)
            TextPainter(
              text: TextSpan(text: label, style: style),
              textScaler: scaler,
              textDirection: direction,
              maxLines: 1,
            )..layout(),
        ];
        try {
          final track = width - inset * 2;
          final count = labels.length;
          double centre(int index) =>
              inset + (count == 1 ? 0 : track * index / (count - 1));
          bool fits(List<int> shown) {
            var end = 0.0;
            for (final index in shown) {
              final size = painters[index].width;
              if (size > width) return false;
              final start = (centre(index) - size / 2).clamp(0.0, width - size);
              if (start < end) return false;
              end = start + size + AppSpacing.sm;
            }
            return true;
          }

          final all = [for (var i = 0; i < count; i++) i];
          final ends = count > 1 ? [0, count - 1] : [0];
          final shown = fits(all) ? all : (fits(ends) ? ends : const <int>[]);
          if (shown.isEmpty) return const SizedBox.shrink();
          final height = painters
              .map((painter) => painter.height)
              .reduce((a, b) => a > b ? a : b);
          return Padding(
            padding: const EdgeInsets.only(top: AppSpacing.xs),
            child: SizedBox(
              height: height,
              child: Stack(
                clipBehavior: Clip.none,
                children: [
                  // The stock slider lays its track out left to right, so
                  // the names follow the same physical positions.
                  for (final index in shown)
                    Positioned(
                      left: (centre(index) - painters[index].width / 2).clamp(
                        0.0,
                        width - painters[index].width,
                      ),
                      top: 0,
                      child: Text(labels[index], style: style, maxLines: 1),
                    ),
                ],
              ),
            ),
          );
        } finally {
          for (final painter in painters) {
            painter.dispose();
          }
        }
      },
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
