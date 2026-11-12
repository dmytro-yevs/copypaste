import 'package:flutter/semantics.dart' show SemanticsRole;
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../app/theme/app_tokens.dart';
import '../l10n/l10n.dart';

/// The steps of one operation and where it stands.
///
/// No text in it wraps. Step names are printed under the markers when each
/// fits on one line there; otherwise one line under the markers reads
/// "1 / 2 · Name", or only the name when that does not fit, and a name wider
/// than the whole line is scaled down to fit. Assistive technology reads the
/// whole indicator as a single element.
class StepProgress extends StatefulWidget {
  const StepProgress({
    super.key,
    required this.steps,
    required this.current,
    this.semanticLabel,
    this.running = true,
    this.value,
    this.valueLabel,
    this.detail,
    this.failure,
  }) : assert(current >= 0);

  final List<String> steps;

  /// The step in progress; [steps] length once every step is complete.
  final int current;
  final String? semanticLabel;

  /// Whether work is under way, which shows the progress bar.
  final bool running;

  /// Completion of the current step from 0 to 1; null when it is not measurable.
  final double? value;

  /// Describe [value] on one line under the bar; [detail] is left out when
  /// both do not fit. While [value] is not measurable the line keeps its
  /// space, so the layout below does not move between steps.
  final String? valueLabel;
  final String? detail;

  /// Marks [current] as failed and names the failure for assistive technology.
  final String? failure;

  @override
  State<StepProgress> createState() => _StepProgressState();
}

class _StepProgressState extends State<StepProgress> {
  final _controller = StepperController();

  @override
  void initState() {
    super.initState();
    _controller.value = _stepperValue;
  }

  @override
  void didUpdateWidget(StepProgress oldWidget) {
    super.didUpdateWidget(oldWidget);
    _controller.value = _stepperValue;
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  StepperValue get _stepperValue => StepperValue(
    currentStep: widget.current,
    stepStates: {if (widget.failure != null) widget.current: StepState.failed},
  );

  int get _named => widget.current.clamp(0, widget.steps.length - 1);

  @override
  Widget build(BuildContext context) {
    final steps = widget.steps;
    final active = widget.running && widget.failure == null;
    final value = active ? widget.value?.clamp(0.0, 1.0) : null;
    final percent = value == null ? null : (value * 100).floor();
    return Semantics(
      container: true,
      // Continuous progress is read on demand; step changes are announced.
      liveRegion: percent == null,
      role: percent != null
          ? SemanticsRole.progressBar
          : active
          ? SemanticsRole.loadingSpinner
          : null,
      label: [
        ?widget.semanticLabel,
        context.l10n.sharedStepSemantics(
          _named + 1,
          steps.length,
          steps[_named],
        ),
        if (percent != null) ?widget.detail,
      ].join(', '),
      value: percent?.toString() ?? widget.failure,
      minValue: percent == null ? null : '0',
      maxValue: percent == null ? null : '100',
      child: ExcludeSemantics(
        child: LayoutBuilder(
          builder: (context, constraints) {
            final width = constraints.maxWidth;
            final named = _namesFit(context, width);
            return Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Stepper(
                  controller: _controller,
                  variant: StepVariant.circleAlt,
                  size: StepSize.small,
                  steps: [
                    for (final (index, name) in steps.indexed)
                      Step(
                        icon: StepNumber(icon: _marker(index)),
                        title: named
                            ? _name(name, index)
                            : const SizedBox.shrink(),
                      ),
                  ],
                ),
                if (!named)
                  IndexedStack(
                    index: _named,
                    alignment: Alignment.center,
                    children: [
                      for (final line in _compactLines(context, width))
                        FittedBox(
                          fit: BoxFit.scaleDown,
                          child: Text(
                            line,
                            maxLines: 1,
                            softWrap: false,
                          ).small().semiBold(),
                        ),
                    ],
                  ),
                if (active) ...[
                  const Gap(AppSpacing.sm),
                  LinearProgressIndicator(value: value),
                  const Gap(AppSpacing.xs),
                  Visibility(
                    visible: value != null,
                    maintainSize: true,
                    maintainAnimation: true,
                    maintainState: true,
                    child: _valueLine(context, width),
                  ),
                ],
              ],
            );
          },
        ),
      ),
    );
  }

  /// The number shrinks to stay inside its marker at large text scales.
  Widget _marker(int index) => Padding(
    padding: const EdgeInsets.all(AppSpacing.xs),
    child: FittedBox(fit: BoxFit.scaleDown, child: Text('${index + 1}')),
  );

  Widget _name(String name, int index) {
    final text = Text(
      name,
      maxLines: 1,
      softWrap: false,
      overflow: TextOverflow.ellipsis,
    );
    if (index == widget.current) return text.semiBold();
    return index > widget.current ? text.muted() : text;
  }

  TextStyle _style(BuildContext context, {bool emphasized = false}) {
    final typography = Theme.of(context).typography;
    final style = DefaultTextStyle.of(context).style.merge(typography.small);
    return emphasized ? style.merge(typography.semiBold) : style;
  }

  double _lineWidth(BuildContext context, String text, TextStyle style) {
    final painter = TextPainter(
      text: TextSpan(text: text, style: style),
      textDirection: Directionality.of(context),
      textScaler: MediaQuery.textScalerOf(context),
      maxLines: 1,
    )..layout();
    final width = painter.width;
    painter.dispose();
    return width;
  }

  bool _namesFit(BuildContext context, double width) {
    if (!width.isFinite) return false;
    final style = _style(context, emphasized: true);
    final cellWidth = width / widget.steps.length - AppSpacing.sm;
    return widget.steps.every(
      (name) => _lineWidth(context, name, style) <= cellWidth,
    );
  }

  List<String> _compactLines(BuildContext context, double width) {
    final steps = widget.steps;
    final positions = [
      for (final (index, name) in steps.indexed)
        context.l10n.sharedStepPosition(index + 1, steps.length, name),
    ];
    final style = _style(context, emphasized: true);
    return positions.every((line) => _lineWidth(context, line, style) <= width)
        ? positions
        : steps;
  }

  Widget _valueLine(BuildContext context, double width) {
    final style = _style(context);
    final label = widget.valueLabel;
    final detail = widget.detail;
    final labelWidth = label == null
        ? 0.0
        : _lineWidth(context, label, style) + AppSpacing.sm;
    final showDetail =
        detail != null &&
        _lineWidth(context, detail, style) + labelWidth <= width;
    return Row(
      children: [
        Expanded(
          child: showDetail
              ? Text(detail, maxLines: 1, softWrap: false).small().muted()
              : const SizedBox.shrink(),
        ),
        if (label != null)
          Text(label, maxLines: 1, softWrap: false).small().muted(),
      ],
    );
  }
}
