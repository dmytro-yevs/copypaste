import 'dart:async';
import 'dart:ui' show lerpDouble;

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../navigation/navigation.dart';
import '../navigation/mobile_navigation_layout.dart';
import '../theme/app_motion.dart';
import '../theme/app_theme.dart';
import '../theme/app_tokens.dart';

/// Stock shadcn navigation with the main-tab geometry and hold gestures.
class AppMobileNavigation extends StatefulWidget {
  const AppMobileNavigation({super.key, required this.controller});
  final AppNavigationController controller;

  @override
  State<AppMobileNavigation> createState() => _AppMobileNavigationState();
}

class _AppMobileNavigationState extends State<AppMobileNavigation>
    with TickerProviderStateMixin {
  late final AppNavigationMotion _motion = AppNavigationMotion(vsync: this);
  MobileNavigationLayout? _layout;
  bool _scrubbing = false;
  bool _settling = false;
  int _gestureEpoch = 0;
  int? _lastPreview;
  Timer? _restoreSelector;

  @override
  void dispose() {
    _restoreSelector?.cancel();
    _motion.dispose();
    super.dispose();
  }

  void _pressTo(double target, double velocity) {
    if (!mounted) return;
    _motion.pressTo(
      target,
      velocity: velocity,
      reduced: AppMotion.reducedMotionOf(context),
    );
  }

  void _activate(AppDestination destination) {
    if (widget.controller.pageTransitionActive ||
        widget.controller.pageGestureActive.value) {
      return;
    }
    unawaited(
      widget.controller.activateDestination(
        destination,
        disableAnimations: AppMotion.reducedMotionOf(context),
      ),
    );
  }

  void _startHold(LongPressStartDetails details) {
    ++_gestureEpoch;
    _restoreSelector?.cancel();
    _motion.selector.stop();
    _lastPreview = widget.controller.selectedDestination.index;
    setState(() {
      _scrubbing = true;
      _settling = false;
    });
    _moveHold(details.localPosition);
    _pressTo(AppMotion.navigationHoldScale, 0);
  }

  void _moveHold(Offset position) {
    final layout = _layout;
    if (layout == null) return;
    _motion.selector.value = layout.positionAt(position.dx);
    final preview = _motion.selector.value.round();
    if (_lastPreview != preview) {
      _lastPreview = preview;
      unawaited(HapticFeedback.selectionClick());
    }
  }

  void _endHold({required bool cancelled}) {
    if (!_scrubbing) return;
    final target = cancelled
        ? widget.controller.selectedDestination.index
        : _motion.selector.value.round();
    final epoch = ++_gestureEpoch;
    setState(() {
      _scrubbing = false;
      _settling = true;
    });
    if (!cancelled) _activate(AppDestination.values[target]);
    _pressTo(1, AppMotion.navigationReleaseVelocity);
    if (AppMotion.reducedMotionOf(context)) {
      _motion.selector.value = target.toDouble();
      setState(() => _settling = false);
    } else {
      _motion.settleSelector(target.toDouble());
      _restoreSelector?.cancel();
      _restoreSelector = Timer(AppMotion.navigationSettle, () {
        if (mounted && epoch == _gestureEpoch) {
          setState(() => _settling = false);
        }
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      key: const ValueKey('mobile-navigation-dock'),
      top: false,
      child: Padding(
        padding: AppTheme.mobileNavigationMargin,
        child: Theme(
          data: AppTheme.mobileNavigationTheme(context),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final layout = MobileNavigationLayout.resolve(
                availableWidth: constraints.maxWidth,
                labels: appNavigationDestinations
                    .map((item) => item.label)
                    .toList(),
                textStyle: AppTheme.mobileNavigationTextStyle(selected: true),
                textScaler: MediaQuery.textScalerOf(context),
                textDirection: Directionality.of(context),
              );
              _layout = layout;
              return Center(
                heightFactor: 1,
                child: SizedBox(
                  width: layout.width,
                  height: layout.height,
                  child: _motion.pressResponse(
                    child: Listener(
                      onPointerDown: (_) => _pressTo(
                        AppMotion.navigationPressScale,
                        AppMotion.navigationPressVelocity,
                      ),
                      onPointerUp: (_) =>
                          _pressTo(1, AppMotion.navigationReleaseVelocity),
                      onPointerCancel: (_) =>
                          _pressTo(1, AppMotion.navigationReleaseVelocity),
                      child: RawGestureDetector(
                        gestures: {
                          LongPressGestureRecognizer:
                              GestureRecognizerFactoryWithHandlers<
                                LongPressGestureRecognizer
                              >(
                                () => LongPressGestureRecognizer(
                                  duration: AppMotion.navigationHold,
                                ),
                                (recognizer) {
                                  recognizer.onLongPressStart = _startHold;
                                  recognizer.onLongPressMoveUpdate = (details) {
                                    _moveHold(details.localPosition);
                                  };
                                  recognizer.onLongPressEnd = (_) =>
                                      _endHold(cancelled: false);
                                  recognizer.onLongPressCancel = () =>
                                      _endHold(cancelled: true);
                                },
                              ),
                        },
                        child: OutlinedContainer(
                          key: const ValueKey('mobile-navigation-surface'),
                          theme: AppTheme.mobileNavigationSurfaceTheme(context),
                          child: AnimatedBuilder(
                            animation: Listenable.merge([
                              widget.controller,
                              widget.controller.visualPosition,
                              widget.controller.pageGestureActive,
                              _motion.selector,
                            ]),
                            builder: (context, _) => _bar(context, layout),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              );
            },
          ),
        ),
      ),
    );
  }

  Widget _bar(BuildContext context, MobileNavigationLayout layout) {
    final dragging = _scrubbing || _settling;
    final position =
        (dragging
                ? _motion.selector.value
                : widget.controller.visualPosition.value)
            .clamp(0.0, (appNavigationDestinations.length - 1).toDouble());
    final lower = position.floor();
    final upper = position.ceil();
    final fraction = position - lower;
    final dragWidth = lerpDouble(
      layout.widths[lower],
      layout.widths[upper],
      fraction,
    )!;
    final dragCenter = lerpDouble(
      layout.centerAt(lower),
      layout.centerAt(upper),
      fraction,
    )!;
    final inset = AppSpacing.xs - AppTheme.mobileNavigationBorderWidth;
    final selected = widget.controller.selectedDestination;
    final pageGesture = widget.controller.pageGestureActive.value;
    final weights = [
      for (var index = 0; index < layout.widths.length; index++)
        dragging || pageGesture
            ? (1 - (position - index).abs()).clamp(0.0, 1.0)
            : selected.index == index
            ? 1.0
            : 0.0,
    ];
    return Stack(
      children: [
        if (dragging)
          Positioned(
            key: const ValueKey('mobile-navigation-drag-indicator'),
            left:
                dragCenter -
                dragWidth / 2 -
                AppTheme.mobileNavigationBorderWidth,
            top: inset,
            width: dragWidth,
            height: layout.itemHeight,
            child: IgnorePointer(
              child: DecoratedBox(
                decoration: AppTheme.mobileNavigationSelectionDecoration(1),
              ),
            ),
          )
        else
          for (var index = 0; index < layout.widths.length; index++)
            Positioned(
              left:
                  layout.centerAt(index) -
                  layout.widths[index] / 2 -
                  AppTheme.mobileNavigationBorderWidth,
              top: inset,
              width: layout.widths[index],
              height: layout.itemHeight,
              child: IgnorePointer(
                child: AppMotion.navigationSelection(
                  context,
                  weight: weights[index],
                  directlyManipulated: dragging || pageGesture,
                  builder: (context, factor, child) => Transform.scale(
                    scale: lerpDouble(
                      AppMotion.navigationMinimumSelectionScale,
                      1,
                      factor,
                    )!,
                    child: DecoratedBox(
                      key: ValueKey('mobile-navigation-selection-$index'),
                      decoration: AppTheme.mobileNavigationSelectionDecoration(
                        factor,
                      ),
                    ),
                  ),
                ),
              ),
            ),
        NavigationBar(
          key: const ValueKey('bottom-navigation'),
          alignment: NavigationBarAlignment.center,
          labelType: NavigationLabelType.all,
          labelPosition: NavigationLabelPosition.bottom,
          labelSize: NavigationLabelSize.small,
          selectedKey: ValueKey(selected),
          onSelected: (key) {
            if (key is ValueKey<AppDestination>) _activate(key.value);
          },
          backgroundColor: Colors.transparent,
          padding: AppTheme.mobileNavigationPadding,
          spacing: AppSpacing.zero,
          children: [
            for (
              var index = 0;
              index < appNavigationDestinations.length;
              index++
            )
              SizedBox(
                width: layout.widths[index],
                height: layout.itemHeight,
                child: AppMotion.navigationSelection(
                  context,
                  weight: weights[index],
                  directlyManipulated: dragging || pageGesture,
                  builder: (context, factor, _) => NavigationItem(
                    key: ValueKey(appNavigationDestinations[index].destination),
                    onChanged: (value) {
                      if (!value) {
                        _activate(appNavigationDestinations[index].destination);
                      }
                    },
                    alignment: Alignment.center,
                    style: AppTheme.mobileNavigationButtonStyle(
                      selected: false,
                      selectionFactor: factor,
                      horizontalPadding: layout.horizontalPadding,
                    ),
                    selectedStyle: AppTheme.mobileNavigationButtonStyle(
                      selected: true,
                      selectionFactor: factor,
                      horizontalPadding: layout.horizontalPadding,
                    ),
                    spacing: AppSpacing.navigationIconLabelGap,
                    overflow: NavigationOverflow.ellipsis,
                    label: Text(
                      appNavigationDestinations[index].label,
                      style: AppTheme.mobileNavigationTextStyle(
                        fontSize: layout.fontSize,
                        selected: weights[index] >= 0.5,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      textAlign: TextAlign.center,
                    ),
                    child: AppMotion.navigationIconResponse(
                      context,
                      selected: weights[index] >= 0.5,
                      child: Icon(
                        appNavigationDestinations[index].icon,
                        size: AppIconSize.lg,
                      ),
                    ),
                  ),
                ),
              ),
          ],
        ),
      ],
    );
  }
}
