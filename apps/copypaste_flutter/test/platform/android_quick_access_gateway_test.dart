import 'package:copypaste_flutter/platform/android/android_quick_access_gateway.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  const channel = MethodChannel('test/quick-access');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  test('decodes the native state and panel option defaults', () async {
    final calls = <MethodCall>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call);
      return {
        'keyboardEnabled': true,
        'keyboardSelected': false,
        'backgroundCapture': false,
        'tileRequestSupported': true,
        'widgetPinSupported': true,
        'widgetCount': 2,
        'panelOptions': {'autoReturn': false},
      };
    });
    const gateway = MethodChannelAndroidQuickAccessGateway(channel: channel);

    final state = await gateway.setPanelOption(
      AndroidPanelOption.autoReturn,
      false,
    );

    expect(calls.single.method, 'setPanelOption');
    expect(calls.single.arguments, {'option': 'autoReturn', 'enabled': false});
    expect(state.keyboardEnabled, isTrue);
    expect(state.widgetCount, 2);
    expect(state.option(AndroidPanelOption.autoReturn), isFalse);
    expect(state.option(AndroidPanelOption.focusSearchOnOpen), isFalse);
  });

  test('maps tile request answers and unknown codes', () async {
    String? answer = 'notAdded';
    messenger.setMockMethodCallHandler(channel, (call) async {
      expect(call.arguments, {'tile': 'captureToggle'});
      return answer;
    });
    const gateway = MethodChannelAndroidQuickAccessGateway(channel: channel);

    expect(
      await gateway.requestAddTile(AndroidQuickAccessTile.captureToggle),
      AndroidTileRequestResult.notAdded,
    );
    answer = null;
    expect(
      await gateway.requestAddTile(AndroidQuickAccessTile.captureToggle),
      AndroidTileRequestResult.unavailable,
    );
  });
}
