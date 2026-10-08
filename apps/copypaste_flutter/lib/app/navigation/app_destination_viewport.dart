import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter/gestures.dart' show PointerDeviceKind;

import '../theme/app_motion.dart';
import 'app_destination.dart';
import 'app_navigation_controller.dart';

/// Retains destination state while coordinating page gestures and tab activation.
class AppDestinationViewport extends StatefulWidget {
  const AppDestinationViewport({
    super.key,
    required this.controller,
    required this.children,
    required this.swipeEnabled,
  });

  final AppNavigationController controller;
  final List<Widget> children;
  final bool swipeEnabled;

  @override
  State<AppDestinationViewport> createState() => _AppDestinationViewportState();
}

class _AppDestinationViewportState extends State<AppDestinationViewport> {
  late final PageController _pages;
  int? _target;
  int _navigationEpoch = 0;
  bool _reportingPage = false;

  @override
  void initState() {
    super.initState();
    _pages = PageController(
      initialPage: widget.controller.selectedDestination.index,
    )..addListener(_reportPosition);
    widget.controller.addListener(_syncSelection);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _syncSelection();
    });
  }

  @override
  void didUpdateWidget(covariant AppDestinationViewport oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_syncSelection);
      widget.controller.addListener(_syncSelection);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _syncSelection();
      });
    }
  }

  void _reportPosition() {
    if (!_pages.hasClients || _reportingPage) return;
    widget.controller.visualPosition.value =
        _pages.page ?? widget.controller.selectedDestination.index.toDouble();
  }

  void _syncSelection() {
    if (!_pages.hasClients || _reportingPage) return;
    final target = widget.controller.selectedDestination.index;
    final page = _pages.page;
    if (page == null || _target == target) return;
    if (_target == null && (page - target).abs() < 0.001) return;
    _target = target;
    widget.controller.pageTransitionActive = true;
    widget.controller.pageGestureActive.value = false;
    final epoch = ++_navigationEpoch;
    final duration = widget.swipeEnabled
        ? AppMotion.resolve(context, AppMotion.navigation)
        : Duration.zero;
    if (duration == Duration.zero || (page - target).abs() < 0.001) {
      _pages.jumpToPage(target);
      _target = null;
      widget.controller.pageTransitionActive = false;
      return;
    }
    unawaited(
      _pages
          .animateToPage(
            target,
            duration: duration,
            curve: AppMotion.navigationCurve,
          )
          .whenComplete(() {
            if (mounted && epoch == _navigationEpoch) {
              _target = null;
              widget.controller.pageTransitionActive = false;
            }
          }),
    );
  }

  void _pageChanged(int index) {
    if (_target != null) return;
    _reportingPage = true;
    widget.controller.selectDestination(AppDestination.values[index]);
    _reportingPage = false;
  }

  @override
  void dispose() {
    widget.controller.pageTransitionActive = false;
    widget.controller.removeListener(_syncSelection);
    _pages.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return NotificationListener<ScrollNotification>(
      onNotification: (notification) {
        if (notification.depth != 0 ||
            notification.metrics.axis != Axis.horizontal) {
          return false;
        }
        if (notification is ScrollStartNotification &&
            notification.dragDetails != null) {
          widget.controller.pageGestureActive.value = true;
        } else if (notification is ScrollEndNotification) {
          widget.controller.pageGestureActive.value = false;
        }
        return false;
      },
      child: PageView(
        key: const ValueKey('destination-pager'),
        controller: _pages,
        allowImplicitScrolling: true,
        scrollBehavior: ScrollConfiguration.of(context).copyWith(
          dragDevices: {
            ...ScrollConfiguration.of(context).dragDevices,
            PointerDeviceKind.mouse,
          },
        ),
        physics: widget.swipeEnabled && !widget.controller.bottomOverlayOpen
            ? const PageScrollPhysics()
            : const NeverScrollableScrollPhysics(),
        onPageChanged: _pageChanged,
        children: [
          for (var index = 0; index < widget.children.length; index++)
            _RetainedDestination(
              key: ValueKey(AppDestination.values[index]),
              child: widget.children[index],
            ),
        ],
      ),
    );
  }
}

class _RetainedDestination extends StatefulWidget {
  const _RetainedDestination({super.key, required this.child});
  final Widget child;

  @override
  State<_RetainedDestination> createState() => _RetainedDestinationState();
}

class _RetainedDestinationState extends State<_RetainedDestination>
    with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return widget.child;
  }
}
