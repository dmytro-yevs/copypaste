import 'package:flutter/services.dart';

abstract interface class QuickPasteWindowHost {
  Future<bool> isSupported();

  Future<void> prepare();

  Future<void> open();

  Future<bool> accessibilityGranted();

  Future<bool> requestAccessibility();

  void setOpenSettingsHandler(VoidCallback? handler);

  Future<void> dispose();
}

class MethodChannelQuickPasteWindowHost implements QuickPasteWindowHost {
  MethodChannelQuickPasteWindowHost({MethodChannel? channel})
    : _channel =
          channel ?? const MethodChannel('com.copypaste.app/quick_paste_host') {
    _channel.setMethodCallHandler(_handleMethodCall);
  }

  final MethodChannel _channel;
  VoidCallback? _openSettings;

  @override
  Future<bool> isSupported() async {
    try {
      return await _channel.invokeMethod<bool>('isSupported') ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  @override
  Future<void> prepare() => _channel.invokeMethod<void>('prepare');

  @override
  Future<void> open() => _channel.invokeMethod<void>('open');

  @override
  Future<bool> accessibilityGranted() async =>
      await _channel.invokeMethod<bool>('accessibilityGranted') ?? false;

  @override
  Future<bool> requestAccessibility() async =>
      await _channel.invokeMethod<bool>('requestAccessibility') ?? false;

  @override
  void setOpenSettingsHandler(VoidCallback? handler) {
    _openSettings = handler;
  }

  Future<Object?> _handleMethodCall(MethodCall call) async {
    if (call.method == 'openSettings') {
      _openSettings?.call();
      return true;
    }
    throw MissingPluginException(
      'Unsupported quick paste method: ${call.method}',
    );
  }

  @override
  Future<void> dispose() async {
    _openSettings = null;
    try {
      await _channel.invokeMethod<void>('dispose');
    } on MissingPluginException {
      // Unsupported and test hosts have no native Quick Paste window.
    } on PlatformException {
      // Process teardown must not be blocked by a late native window error.
    }
    _channel.setMethodCallHandler(null);
  }
}

abstract interface class QuickPasteContextHost {
  Future<bool> accessibilityGranted();

  Future<bool> requestAccessibility();

  Future<bool> paste();

  Future<void> close();

  Future<void> openMainWindow();

  Future<void> openSettings();

  Future<void> quit();

  void setOpenedHandler(Future<void> Function()? handler);

  Future<void> dispose();
}

class MethodChannelQuickPasteContextHost implements QuickPasteContextHost {
  MethodChannelQuickPasteContextHost({MethodChannel? channel})
    : _channel =
          channel ??
          const MethodChannel('com.copypaste.app/quick_paste_context') {
    _channel.setMethodCallHandler(_handleMethodCall);
  }

  final MethodChannel _channel;
  Future<void> Function()? _opened;

  @override
  Future<bool> accessibilityGranted() async =>
      await _channel.invokeMethod<bool>('accessibilityGranted') ?? false;

  @override
  Future<bool> requestAccessibility() async =>
      await _channel.invokeMethod<bool>('requestAccessibility') ?? false;

  @override
  Future<bool> paste() async =>
      await _channel.invokeMethod<bool>('paste') ?? false;

  @override
  Future<void> close() => _channel.invokeMethod<void>('close');

  @override
  Future<void> openMainWindow() => _channel.invokeMethod<void>('openMain');

  @override
  Future<void> openSettings() => _channel.invokeMethod<void>('openSettings');

  @override
  Future<void> quit() => _channel.invokeMethod<void>('quit');

  @override
  void setOpenedHandler(Future<void> Function()? handler) {
    _opened = handler;
  }

  Future<Object?> _handleMethodCall(MethodCall call) async {
    if (call.method == 'opened') {
      await _opened?.call();
      return true;
    }
    throw MissingPluginException(
      'Unsupported quick paste method: ${call.method}',
    );
  }

  @override
  Future<void> dispose() async {
    _opened = null;
    _channel.setMethodCallHandler(null);
  }
}
