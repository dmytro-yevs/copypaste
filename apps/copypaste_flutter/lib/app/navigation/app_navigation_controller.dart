import 'package:flutter/widgets.dart';

import 'app_destination.dart';
import 'app_page_route.dart';
import '../theme/app_motion.dart';
import '../theme/app_overlays.dart';

/// Owns shell destination selection and the nested content navigation stack.
class AppNavigationController extends ChangeNotifier {
  AppNavigationController({
    AppDestination initialDestination = AppDestination.history,
  }) : _selectedDestination = initialDestination,
       visualPosition = ValueNotifier<double>(
         initialDestination.index.toDouble(),
       );

  final GlobalKey<NavigatorState> navigatorKey = GlobalKey<NavigatorState>();
  final ValueNotifier<double> visualPosition;
  final ValueNotifier<bool> pageGestureActive = ValueNotifier(false);
  final ValueNotifier<bool> historyFindRequests = ValueNotifier(false);
  bool pageTransitionActive = false;
  final Map<AppDestination, ScrollController> _scrollControllers = {};

  ScrollController scrollControllerFor(AppDestination destination) =>
      _scrollControllers.putIfAbsent(destination, ScrollController.new);

  /// A repeated activation returns to the root and scrolls its main list up.
  Future<void> activateDestination(
    AppDestination destination, {
    bool disableAnimations = false,
  }) async {
    if (destination != _selectedDestination) {
      selectDestination(destination);
      return;
    }
    navigatorKey.currentState?.popUntil((route) => route.isFirst);
    final controller = _scrollControllers[destination];
    if (controller == null || !controller.hasClients) return;
    final duration = AppMotion.resolveDisabled(
      disableAnimations,
      AppMotion.navigation,
    );
    await Future.wait([
      for (final position in controller.positions.toList())
        if (duration == Duration.zero)
          Future<void>.sync(() => position.jumpTo(position.minScrollExtent))
        else
          position.animateTo(
            position.minScrollExtent,
            duration: duration,
            curve: AppMotion.navigationCurve,
          ),
    ]);
  }

  @override
  void dispose() {
    visualPosition.dispose();
    pageGestureActive.dispose();
    historyFindRequests.dispose();
    for (final controller in _scrollControllers.values) {
      controller.dispose();
    }
    super.dispose();
  }

  AppDestination _selectedDestination;
  bool _bottomOverlayOpen = false;
  int _visibleDrawers = 0;

  AppDestination get selectedDestination => _selectedDestination;

  void findInHistory() {
    selectDestination(AppDestination.history);
    historyFindRequests.value = true;
  }

  bool get bottomOverlayOpen => _bottomOverlayOpen || _visibleDrawers > 0;

  final _NestedRouteStack _routes = _NestedRouteStack();

  /// Observes the nested navigator; the navigation host attaches it.
  NavigatorObserver get routeObserver => _routes;

  /// Forgets the routes of a disposed nested navigator.
  void releaseRoutes() => _routes.clear();

  /// Whether the open drawer and every nested route can close without losing
  /// work. Overlays not marked passive never are.
  bool get overlaysPassive {
    final context = navigatorKey.currentContext;
    return !_bottomOverlayOpen &&
        (context == null || AppOverlays.drawerPassiveOf(context)) &&
        _routes.pushed.every(
          (route) => route is AppPageRoute && (route.passive?.call() ?? false),
        );
  }

  /// Closes the overlays that [overlaysPassive] reports as passive.
  void dismissPassiveOverlays() {
    final context = navigatorKey.currentContext;
    if (context != null) AppOverlays.closePassiveDrawer(context);
    navigatorKey.currentState?.popUntil((route) => route.isFirst);
  }

  void handleDrawerVisibilityChanged(bool visible) {
    final wasOpen = bottomOverlayOpen;
    if (visible) {
      _visibleDrawers++;
    } else if (_visibleDrawers > 0) {
      _visibleDrawers--;
    }
    // A replaced drawer completes after its successor has already opened.
    if (wasOpen != bottomOverlayOpen) notifyListeners();
  }

  void setBottomOverlayOpen(bool open) {
    if (_bottomOverlayOpen == open) return;
    _bottomOverlayOpen = open;
    notifyListeners();
  }

  void selectDestination(AppDestination destination) {
    if (_selectedDestination == destination) {
      return;
    }

    navigatorKey.currentState?.popUntil(
      (Route<dynamic> route) => route.isFirst,
    );
    _selectedDestination = destination;
    notifyListeners();
  }

  /// Pushes a nested detail route without changing the shell tab.
  ///
  /// Standard Shad dialogs use the root navigator and handle Back and Escape
  /// before this nested stack.
  Future<T?> push<T>(
    BuildContext context, {
    required WidgetBuilder builder,
    RouteSettings settings = const RouteSettings(),
  }) {
    final NavigatorState? navigator = navigatorKey.currentState;
    if (navigator == null) {
      throw StateError('The app navigation controller is not attached.');
    }

    return navigator.push<T>(
      AppPageRoute<T>(
        builder: builder,
        settings: settings,
        disableAnimations: MediaQuery.disableAnimationsOf(context),
      ),
    );
  }

  /// Pops the topmost overlay or nested route, if one is present.
  Future<bool> maybePop<T extends Object?>([T? result]) {
    return navigatorKey.currentState?.maybePop<T>(result) ??
        Future<bool>.value(false);
  }

  /// Handles Escape without requesting an application or window close.
  Future<bool> handleEscape() async {
    if (await maybePop<void>()) return true;
    if (_selectedDestination == AppDestination.history) return false;
    selectDestination(AppDestination.history);
    return true;
  }
}

class _NestedRouteStack extends NavigatorObserver {
  final List<Route<dynamic>> _routes = [];

  /// Routes above the destinations' root that are still on the stack.
  Iterable<Route<dynamic>> get pushed =>
      _routes.where((route) => route.isActive && !route.isFirst);

  void clear() => _routes.clear();

  void _forget(Route<dynamic>? route) {
    _routes.removeWhere((entry) => entry == route || !entry.isActive);
  }

  @override
  void didPush(Route<dynamic> route, Route<dynamic>? previousRoute) {
    _forget(null);
    _routes.add(route);
  }

  @override
  void didPop(Route<dynamic> route, Route<dynamic>? previousRoute) =>
      _forget(route);

  @override
  void didRemove(Route<dynamic> route, Route<dynamic>? previousRoute) =>
      _forget(route);

  @override
  void didReplace({Route<dynamic>? newRoute, Route<dynamic>? oldRoute}) {
    _forget(oldRoute);
    if (newRoute != null) _routes.add(newRoute);
  }
}
