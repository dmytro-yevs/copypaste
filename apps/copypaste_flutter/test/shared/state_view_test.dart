import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/shell/app_shell.dart';
import 'package:copypaste_flutter/shared/state_view.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets(
    'mobile state views keep their final content above floating navigation',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(320, 400));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      var retries = 0;
      for (final state in [
        const StateView.loading(),
        StateView.empty(
          title: 'No clips',
          actionLabel: 'Capture',
          onAction: () => retries++,
        ),
        StateView.error(
          title: 'History is unavailable',
          message: 'Check the connection and try again.',
          actionLabel: 'Try again',
          onAction: () => retries++,
        ),
      ]) {
        final navigation = AppNavigationController();
        addTearDown(navigation.dispose);
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            builder: (context, child) => AppTheme.builder(
              context,
              MediaQuery(
                data: MediaQuery.of(context).copyWith(
                  padding: const EdgeInsets.only(bottom: 24),
                  viewPadding: const EdgeInsets.only(bottom: 24),
                  textScaler: const TextScaler.linear(2),
                ),
                child: child!,
              ),
            ),
            home: AppShell(
              controller: navigation,
              destinations: {
                AppDestination.history: state,
                AppDestination.devices: const SizedBox.expand(),
                AppDestination.settings: const SizedBox.expand(),
              },
            ),
          ),
        );
        await tester.pump();
        final position = tester
            .state<ScrollableState>(
              find.descendant(
                of: find.byType(StateView),
                matching: find.byType(Scrollable),
              ),
            )
            .position;
        position.jumpTo(position.maxScrollExtent);
        await tester.pump();
        final content = state.kind == StateViewKind.loading
            ? find.text('Loading')
            : find.widgetWithText(Button, state.actionLabel!);
        expect(
          tester.getRect(content).bottom,
          lessThan(
            tester
                .getRect(find.byKey(const ValueKey('mobile-navigation-dock')))
                .top,
          ),
        );
        if (state.kind != StateViewKind.loading) await tester.tap(content);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      }
      expect(retries, 2);
    },
    variant: TargetPlatformVariant({
      TargetPlatform.android,
      TargetPlatform.macOS,
      TargetPlatform.windows,
    }),
  );

  Widget buildSubject(Widget child) {
    return ShadcnApp(
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: AppTheme.mode,
      builder: AppTheme.builder,
      home: Align(alignment: Alignment.topLeft, child: child),
    );
  }

  testWidgets('uses the shadcn loading indicator', (tester) async {
    await tester.pumpWidget(buildSubject(const StateView.loading()));

    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    expect(find.text('Loading'), findsOneWidget);
  });

  testWidgets('supports a compact empty state without explanatory copy', (
    tester,
  ) async {
    await tester.pumpWidget(
      buildSubject(
        const StateView.empty(
          title: 'No devices nearby',
          compact: true,
          icon: LucideIcons.monitorSmartphone,
        ),
      ),
    );
    expect(find.text('No devices nearby'), findsOneWidget);
    final icon = tester.widget<Icon>(
      find.byIcon(LucideIcons.monitorSmartphone),
    );
    expect(icon.size, 24);
    expect(find.byType(Button), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('keeps functional loading progress with reduced motion', (
    tester,
  ) async {
    await tester.pumpWidget(
      buildSubject(
        const MediaQuery(
          data: MediaQueryData(disableAnimations: true),
          child: StateView.loading(),
        ),
      ),
    );

    final indicator = tester.widget<CircularProgressIndicator>(
      find.byType(CircularProgressIndicator),
    );
    expect(indicator.value, isNull);
  });

  testWidgets('uses the shadcn destructive alert for recovery', (tester) async {
    var retries = 0;
    await tester.pumpWidget(
      buildSubject(
        StateView.error(
          title: 'Sync failed',
          message: 'Check your connection and try again.',
          actionLabel: 'Try again',
          onAction: () => retries += 1,
        ),
      ),
    );

    expect(find.byType(Alert), findsOneWidget);
    expect(find.widgetWithText(Button, 'Try again'), findsOneWidget);
    await tester.tap(find.text('Try again'));
    expect(retries, 1);
  });

  testWidgets('wraps a long empty message at 320 logical pixels and 2x text', (
    tester,
  ) async {
    await tester.pumpWidget(
      buildSubject(
        SizedBox(
          width: 320,
          height: 720,
          child: MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(2)),
            child: const StateView.empty(
              title: 'Nothing copied yet',
              message:
                  'Copied text, links, and images will appear here when they are available.',
            ),
          ),
        ),
      ),
    );

    expect(find.text('Nothing copied yet'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'keeps a long error and recovery action reachable in a short view',
    (tester) async {
      var retries = 0;
      await tester.pumpWidget(
        buildSubject(
          SizedBox(
            width: 320,
            height: 180,
            child: MediaQuery(
              data: const MediaQueryData(textScaler: TextScaler.linear(2)),
              child: StateView.error(
                title: 'Synchronization requires your attention',
                message:
                    'The connection was interrupted before your encrypted clipboard history could be synchronized. Check your network connection, then try again.',
                actionLabel:
                    'Reconnect this device and try synchronization again',
                onAction: () => retries += 1,
              ),
            ),
          ),
        ),
      );

      expect(find.byType(Alert), findsOneWidget);
      expect(find.byType(SingleChildScrollView), findsOneWidget);
      final scrollable = tester.state<ScrollableState>(
        find.descendant(
          of: find.byType(SingleChildScrollView),
          matching: find.byType(Scrollable),
        ),
      );
      expect(scrollable.position.maxScrollExtent, greaterThan(0));
      final action = find.widgetWithText(
        Button,
        'Reconnect this device and try synchronization again',
      );
      final actionBounds = tester.getRect(action);
      final viewportBounds = tester.getRect(find.byType(SingleChildScrollView));
      scrollable.position.jumpTo(
        (actionBounds.center.dy - viewportBounds.center.dy)
            .clamp(0.0, scrollable.position.maxScrollExtent)
            .toDouble(),
      );
      await tester.pump();
      await tester.tap(action);
      expect(retries, 1);
      expect(tester.takeException(), isNull);
    },
  );
}
