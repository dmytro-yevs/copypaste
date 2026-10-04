import 'package:flutter/widgets.dart';

import 'app_destination.dart';
import 'app_page_route.dart';

/// Owns shell destination selection and the nested content navigation stack.
class AppNavigationController extends ChangeNotifier {
  AppNavigationController({
    AppDestination initialDestination = AppDestination.history,
  }) : _selectedDestination = initialDestination;

  final GlobalKey<NavigatorState> navigatorKey = GlobalKey<NavigatorState>();

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
  Future<bool> handleEscape() => maybePop<void>();
}
