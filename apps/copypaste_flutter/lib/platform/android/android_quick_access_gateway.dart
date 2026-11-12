import 'package:flutter/services.dart';

/// Device-local keyboard panel options the panel reads natively.
enum AndroidPanelOption {
  autoReturn('autoReturn', true),
  focusSearchOnOpen('focusSearchOnOpen', false);

  const AndroidPanelOption(this.wire, this.defaultValue);

  final String wire;
  final bool defaultValue;
}

enum AndroidQuickAccessTile {
  saveClipboard('saveClipboard'),
  captureToggle('captureToggle');

  const AndroidQuickAccessTile(this.wire);

  final String wire;
}

/// The system's answer to a Quick Settings placement prompt.
enum AndroidTileRequestResult {
  added,
  alreadyAdded,
  notAdded,

  /// The prompt could not be shown (older Android or a system error).
  unavailable;

  static AndroidTileRequestResult fromWire(String? wire) => values.firstWhere(
    (value) => value.name == wire,
    orElse: () => unavailable,
  );
}

class AndroidQuickAccessState {
  const AndroidQuickAccessState({
    required this.keyboardEnabled,
    required this.keyboardSelected,
    required this.backgroundCapture,
    required this.tileRequestSupported,
    required this.widgetPinSupported,
    required this.widgetCount,
    required this.panelOptions,
  });

  /// The clips keyboard is turned on in the system keyboard settings.
  final bool keyboardEnabled;

  /// The clips keyboard is the current input method.
  final bool keyboardSelected;

  /// Shizuku- or ADB-granted background capture is running.
  final bool backgroundCapture;

  /// Android 13+ can prompt to place a Quick Settings tile.
  final bool tileRequestSupported;
  final bool widgetPinSupported;
  final int widgetCount;
  final Map<AndroidPanelOption, bool> panelOptions;

  bool option(AndroidPanelOption option) =>
      panelOptions[option] ?? option.defaultValue;
}

abstract interface class AndroidQuickAccessGateway {
  Future<AndroidQuickAccessState> state();

  /// Opens the system screen where input methods are turned on.
  Future<bool> openKeyboardSettings();

  /// Shows the system input method picker; false when the keyboard is off.
  Future<bool> showKeyboardPicker();

  Future<AndroidTileRequestResult> requestAddTile(AndroidQuickAccessTile tile);

  /// Asks the launcher to pin the widget; false when it cannot.
  Future<bool> requestPinWidget();

  Future<AndroidQuickAccessState> setPanelOption(
    AndroidPanelOption option,
    bool enabled,
  );
}

class MethodChannelAndroidQuickAccessGateway
    implements AndroidQuickAccessGateway {
  const MethodChannelAndroidQuickAccessGateway({
    MethodChannel channel = const MethodChannel(
      'com.copypaste.app/quick_access',
    ),
  }) : _channel = channel;

  final MethodChannel _channel;

  @override
  Future<AndroidQuickAccessState> state() => _state('state');

  @override
  Future<bool> openKeyboardSettings() async =>
      await _channel.invokeMethod<bool>('openKeyboardSettings') ?? false;

  @override
  Future<bool> showKeyboardPicker() async =>
      await _channel.invokeMethod<bool>('showKeyboardPicker') ?? false;

  @override
  Future<AndroidTileRequestResult> requestAddTile(
    AndroidQuickAccessTile tile,
  ) async => AndroidTileRequestResult.fromWire(
    await _channel.invokeMethod<String>('requestAddTile', {'tile': tile.wire}),
  );

  @override
  Future<bool> requestPinWidget() async =>
      await _channel.invokeMethod<bool>('requestPinWidget') ?? false;

  @override
  Future<AndroidQuickAccessState> setPanelOption(
    AndroidPanelOption option,
    bool enabled,
  ) => _state('setPanelOption', {'option': option.wire, 'enabled': enabled});

  Future<AndroidQuickAccessState> _state(
    String method, [
    Map<String, Object?>? arguments,
  ]) async {
    final raw = await _channel.invokeMapMethod<String, Object?>(
      method,
      arguments,
    );
    if (raw == null) throw StateError('Quick access state is unavailable.');
    return _decode(raw);
  }

  static AndroidQuickAccessState _decode(Map<String, Object?> raw) {
    final options = Map<Object?, Object?>.from(
      raw['panelOptions'] as Map<Object?, Object?>? ?? const {},
    );
    return AndroidQuickAccessState(
      keyboardEnabled: raw['keyboardEnabled'] as bool? ?? false,
      keyboardSelected: raw['keyboardSelected'] as bool? ?? false,
      backgroundCapture: raw['backgroundCapture'] as bool? ?? false,
      tileRequestSupported: raw['tileRequestSupported'] as bool? ?? false,
      widgetPinSupported: raw['widgetPinSupported'] as bool? ?? false,
      widgetCount: (raw['widgetCount'] as num?)?.toInt() ?? 0,
      panelOptions: Map.unmodifiable({
        for (final option in AndroidPanelOption.values)
          option: options[option.wire] as bool? ?? option.defaultValue,
      }),
    );
  }
}
