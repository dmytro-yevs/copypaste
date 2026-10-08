import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'app_destination.dart';
import 'app_navigation_controller.dart';
import 'app_page_route.dart';

/// Hosts the persistent nested navigator used by every adaptive shell layout.
class AppNavigationHost extends StatefulWidget {
  const AppNavigationHost({
    super.key,
    required this.controller,
    required this.child,
  });

  final AppNavigationController controller;
  final Widget child;

  @override
  State<AppNavigationHost> createState() => _AppNavigationHostState();
}

class _AppNavigationHostState extends State<AppNavigationHost> {
  late final ValueNotifier<Widget> _child = ValueNotifier<Widget>(widget.child);
  bool _nestedCanPop = false;

  @override
  void didUpdateWidget(covariant AppNavigationHost oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.child != widget.child) {
      _child.value = widget.child;
    }
  }

  @override
  void dispose() {
    _child.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.controller,
      builder: (context, child) {
        final interceptRootBack =
            !_nestedCanPop &&
            widget.controller.selectedDestination != AppDestination.history;
        return PopScope<Object?>(
          canPop: !interceptRootBack,
          onPopInvokedWithResult: (didPop, result) {
            if (!didPop && interceptRootBack) {
              widget.controller.selectDestination(AppDestination.history);
            }
          },
          child: child!,
        );
      },
      child: NavigatorPopHandler<Object?>(
        onPopWithResult: (Object? result) {
          widget.controller.maybePop<Object?>(result);
        },
        child: NotificationListener<NavigationNotification>(
          onNotification: (notification) {
            if (_nestedCanPop != notification.canHandlePop) {
              setState(() => _nestedCanPop = notification.canHandlePop);
            }
            return false;
          },
          child: Navigator(
            key: widget.controller.navigatorKey,
            onGenerateRoute: (RouteSettings settings) {
              return AppPageRoute<void>(
                settings: settings,
                disableAnimations: MediaQuery.disableAnimationsOf(context),
                builder: (BuildContext context) {
                  return ValueListenableBuilder<Widget>(
                    valueListenable: _child,
                    builder: (BuildContext context, Widget child, Widget? _) {
                      return child;
                    },
                  );
                },
              );
            },
          ),
        ),
      ),
    );
  }
}
