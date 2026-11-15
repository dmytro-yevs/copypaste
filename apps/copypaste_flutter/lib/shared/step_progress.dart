import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../app/theme/app_motion.dart';
import '../app/theme/app_tokens.dart';
import '../l10n/l10n.dart';

/// The steps of one operation and where it stands.
///
/// One line names the current step with "Step 2 of 4" beside it, and a row of
/// small markers joined by lines shows every step: a check for finished
/// steps, a dot for the current one and a cross when it failed. Nothing in it
/// wraps, so its height never changes between steps. Assistive technology
/// reads the whole indicator as a single element and hears each step change.
class StepProgress extends StatelessWidget {
  const StepProgress({
    super.key,
    required this.steps,
    required this.current,
    this.semanticLabel,
    this.failure,
  }) : assert(current >= 0);

  final List<String> steps;

  /// The step in progress; [steps] length once every step is complete.
  final int current;
  final String? semanticLabel;

  /// Marks [current] as failed and names the failure for assistive technology.
  final String? failure;

  int get _named => current.clamp(0, steps.length - 1);

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final named = _named;
    return Semantics(
      container: true,
      liveRegion: true,
      label: [
        ?semanticLabel,
        l10n.sharedStepSemantics(named + 1, steps.length, steps[named]),
      ].join(', '),
      value: failure,
      child: ExcludeSemantics(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _StepHeading(
              name: steps[named],
              counter: l10n.sharedStepCounter(named + 1, steps.length),
              failed: failure != null,
            ),
            const Gap(AppSpacing.sm),
            Row(
              children: [
                for (var index = 0; index < steps.length; index++) ...[
                  _StepMarker(state: _stateOf(index)),
                  if (index < steps.length - 1)
                    Expanded(child: _StepConnector(done: index < current)),
                ],
              ],
            ),
          ],
        ),
      ),
    );
  }

  _StepState _stateOf(int index) {
    if (index < current) return _StepState.done;
    if (index > current) return _StepState.upcoming;
    return failure == null ? _StepState.current : _StepState.failed;
  }
}

enum _StepState { done, current, failed, upcoming }

/// The current step's name and its position, each kept to one line.
class _StepHeading extends StatelessWidget {
  const _StepHeading({
    required this.name,
    required this.counter,
    required this.failed,
  });

  final String name;
  final String counter;
  final bool failed;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final title = Text(
      name,
      maxLines: 1,
      softWrap: false,
      overflow: TextOverflow.ellipsis,
    ).small().semiBold();
    return LayoutBuilder(
      // The position never takes more than half the line from the name.
      builder: (context, constraints) => Row(
        children: [
          Expanded(
            child: failed
                ? DefaultTextStyle.merge(
                    style: TextStyle(color: colors.destructive),
                    child: title,
                  )
                : title,
          ),
          const Gap(AppSpacing.sm),
          ConstrainedBox(
            constraints: BoxConstraints(
              maxWidth: constraints.maxWidth.isFinite
                  ? constraints.maxWidth / 2
                  : double.infinity,
            ),
            child: Text(
              counter,
              maxLines: 1,
              softWrap: false,
              overflow: TextOverflow.ellipsis,
            ).small().muted(),
          ),
        ],
      ),
    );
  }
}

class _StepMarker extends StatelessWidget {
  const _StepMarker({required this.state});

  final _StepState state;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final (fill, outline) = switch (state) {
      _StepState.done => (colors.primary, colors.primary),
      _StepState.failed => (colors.background, colors.destructive),
      _StepState.current => (colors.background, colors.primary),
      _StepState.upcoming => (colors.background, colors.border),
    };
    final child = switch (state) {
      _StepState.done => Icon(
        LucideIcons.check,
        size: AppStepSize.markerIcon,
        color: colors.primaryForeground,
      ),
      _StepState.failed => Icon(
        LucideIcons.x,
        size: AppStepSize.markerIcon,
        color: colors.destructive,
      ),
      _StepState.current => Container(
        width: AppStepSize.currentDot,
        height: AppStepSize.currentDot,
        decoration: BoxDecoration(
          color: colors.primary,
          shape: BoxShape.circle,
        ),
      ),
      _StepState.upcoming => null,
    };
    return AnimatedContainer(
      duration: AppMotion.resolve(context, AppMotion.standard),
      width: AppStepSize.marker,
      height: AppStepSize.marker,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: fill,
        shape: BoxShape.circle,
        border: Border.all(color: outline, width: AppStepSize.markerBorder),
      ),
      child: child,
    );
  }
}

class _StepConnector extends StatelessWidget {
  const _StepConnector({required this.done});

  final bool done;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return AnimatedContainer(
      duration: AppMotion.resolve(context, AppMotion.standard),
      height: AppStepSize.connector,
      margin: const EdgeInsets.symmetric(horizontal: AppSpacing.xs),
      decoration: BoxDecoration(
        color: done ? colors.primary : colors.border,
        borderRadius: BorderRadius.circular(AppRadius.full),
      ),
    );
  }
}
