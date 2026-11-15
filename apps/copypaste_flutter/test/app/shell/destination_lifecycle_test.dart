import 'package:copypaste_flutter/app/navigation/app_destination.dart';
import 'package:copypaste_flutter/app/navigation/app_destination_viewport.dart';
import 'package:copypaste_flutter/app/navigation/app_navigation_controller.dart';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter_test/flutter_test.dart';

class _Probe extends StatefulWidget {
  const _Probe({required this.ticks, required this.label});
  final ValueNotifier<int> ticks;
  final ValueNotifier<String> label;

  @override
  State<_Probe> createState() => _ProbeState();
}

class _ProbeState extends State<_Probe> with SingleTickerProviderStateMixin {
  late final Ticker ticker;
  int localState = 0;

  @override
  void initState() {
    super.initState();
    ticker = createTicker((_) => widget.ticks.value++)..start();
  }

  @override
  void dispose() {
    ticker.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => GestureDetector(
    onTap: () => setState(() => localState++),
    child: ValueListenableBuilder<String>(
      valueListenable: widget.label,
      builder: (context, label, _) => Text('$label:$localState'),
    ),
  );
}

void main() {
  testWidgets('inactive destination stops ticking and retains current state', (
    tester,
  ) async {
    final navigation = AppNavigationController();
    final ticks = ValueNotifier(0);
    final label = ValueNotifier('old');
    addTearDown(navigation.dispose);
    addTearDown(ticks.dispose);
    addTearDown(label.dispose);
    await tester.pumpWidget(
      MaterialApp(
        home: AppDestinationViewport(
          controller: navigation,
          swipeEnabled: false,
          children: [
            _Probe(ticks: ticks, label: label),
            const Text('devices'),
            const Text('settings'),
          ],
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 20));
    await tester.tap(find.text('old:0'));
    await tester.pump(const Duration(milliseconds: 20));
    expect(find.text('old:1'), findsOneWidget);
    navigation.selectDestination(AppDestination.devices);
    await tester.pump();
    await tester.pump();
    final stopped = ticks.value;
    label.value = 'new';
    await tester.pump(const Duration(milliseconds: 100));
    expect(ticks.value, stopped);
    navigation.selectDestination(AppDestination.history);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    expect(find.text('new:1'), findsOneWidget);
    expect(ticks.value, greaterThan(stopped));
    await tester.pumpWidget(const SizedBox());
  });
}
