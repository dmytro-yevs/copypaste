import 'package:flutter/widgets.dart';

import 'app_destination.dart';
import 'app_page_route.dart';
import '../theme/app_motion.dart';

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
    for (final controller in _scrollControllers.values) {
      controller.dispose();
    }
    super.dispose();
  }

  AppDestination _selectedDestination;
  bool _bottomOverlayOpen = false;

  AppDestination get selectedDestination => _selectedDestination;
  bool get bottomOverlayOpen => _bottomOverlayOpen;

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
