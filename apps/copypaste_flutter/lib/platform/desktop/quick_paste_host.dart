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
  Future<void> prepare() async {
    if (await _channel.invokeMethod<bool>('prepare') != true) {
      throw PlatformException(code: 'window_unavailable');
    }
  }

  @override
  Future<void> open() async {
    final result = await _channel.invokeMethod<bool>('open');
    if (result != true) {
      throw PlatformException(code: 'window_unavailable');
    }
  }

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

typedef QuickPasteOpenedHandler =
    Future<void> Function(int presentationId, {required bool inspectorVisible});

abstract interface class QuickPasteContextHost {
  Future<void> signalReady();

  Future<bool> accessibilityGranted();

  Future<bool> requestAccessibility();

  Future<void> setInspectorVisible({
    required int presentationId,
    required bool visible,
  });

  Future<bool> paste({required int presentationId});

  Future<void> close({required int presentationId});

  Future<void> openMainWindow();

  Future<void> openSettings();

  Future<void> quit();

  void setOpenedHandler(QuickPasteOpenedHandler? handler);

  void setShutdownHandler(Future<void> Function()? handler);

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
  QuickPasteOpenedHandler? _opened;
  Future<void> Function()? _shutdown;

  @override
  Future<void> signalReady() async {
    if (await _channel.invokeMethod<bool>('ready') != true) {
      throw PlatformException(code: 'context_unavailable');
    }
  }

  @override
  Future<bool> accessibilityGranted() async =>
      await _channel.invokeMethod<bool>('accessibilityGranted') ?? false;

  @override
  Future<bool> requestAccessibility() async =>
      await _channel.invokeMethod<bool>('requestAccessibility') ?? false;

  @override
  Future<void> setInspectorVisible({
    required int presentationId,
    required bool visible,
  }) async {
    if (await _channel.invokeMethod<bool>('setInspectorVisible', {
          'presentationId': presentationId,
          'visible': visible,
        }) !=
        true) {
      throw PlatformException(code: 'window_unavailable');
    }
  }

  @override
  Future<bool> paste({required int presentationId}) async {
    try {
      return await _channel.invokeMethod<Object?>('paste', {
            'presentationId': presentationId,
          }) ==
          true;
    } on PlatformException {
      return false;
    } on MissingPluginException {
      return false;
    }
  }

  @override
  Future<void> close({required int presentationId}) =>
      _channel.invokeMethod<void>('close', {'presentationId': presentationId});

  @override
  Future<void> openMainWindow() => _channel.invokeMethod<void>('openMain');

  @override
  Future<void> openSettings() => _channel.invokeMethod<void>('openSettings');

  @override
  Future<void> quit() => _channel.invokeMethod<void>('quit');

  @override
  void setOpenedHandler(QuickPasteOpenedHandler? handler) {
    _opened = handler;
  }

  @override
  void setShutdownHandler(Future<void> Function()? handler) {
    _shutdown = handler;
  }

  Future<Object?> _handleMethodCall(MethodCall call) async {
    if (call.method == 'shutdown') {
      await _shutdown?.call();
      return true;
    }
    if (call.method == 'opened') {
      final arguments = call.arguments;
      final id = arguments is Map ? arguments['presentationId'] : null;
      if (id is! int || id <= 0 || id > 0x7fffffffffffffff) {
        throw PlatformException(code: 'invalid_presentation');
      }
      final inspectorVisible = arguments['inspectorVisible'] ?? false;
      if (inspectorVisible is! bool) {
        throw PlatformException(code: 'invalid_presentation');
      }
      await _opened?.call(id, inspectorVisible: inspectorVisible);
      return true;
    }
    throw MissingPluginException(
      'Unsupported quick paste method: ${call.method}',
    );
  }

  @override
  Future<void> dispose() async {
    _opened = null;
    _shutdown = null;
    _channel.setMethodCallHandler(null);
  }
}
