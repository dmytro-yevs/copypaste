import 'package:flutter/services.dart';

/// Tray bounds use native global screen coordinates with a top-left origin.
Future<void> openTrayQuickPaste(
  Rect anchor, {
  MethodChannel channel = const MethodChannel(
    'com.copypaste.app/quick_paste_host',
  ),
}) async {
  if (!anchor.left.isFinite ||
      !anchor.top.isFinite ||
      !anchor.width.isFinite ||
      !anchor.height.isFinite ||
      anchor.isEmpty) {
    throw ArgumentError.value(anchor, 'anchor', 'Invalid tray bounds');
  }
  final result = await channel.invokeMethod<bool>('openFromTray', {
    'x': anchor.left,
    'y': anchor.top,
    'width': anchor.width,
    'height': anchor.height,
  });
  if (result != true) {
    throw PlatformException(code: 'window_unavailable');
  }
}

/// Result of pasting the clipboard into the foreground application.
///
/// Every outcome except [pasted] leaves the content on the clipboard, so the
/// caller can tell the user to paste manually.
enum PasteOutcome {
  pasted,

  /// Synthesized input is blocked: no Accessibility permission, or secure
  /// input is active.
  copiedOnly,

  /// The foreground window runs at a higher integrity level (Windows).
  elevatedTarget,

  /// No application can receive the paste.
  noTarget,

  /// The session cannot synthesize input, for example Wayland without an
  /// input-injection path.
  unsupported,
}

/// Native access to the application that receives a paste.
///
/// Channel contract: `pasteForeground {plainText: bool}` answers a
/// [PasteOutcome] name, `moveCaretLeft {count: int}` a bool, and
/// `secureFieldFocused` a bool or null.
abstract interface class ForegroundPasteHost {
  /// The largest caret move a paste may request.
  static const maxCaretMove = 2000;

  /// [plainText] asks for the plain-text paste where the platform has one.
  Future<PasteOutcome> pasteForeground({bool plainText = false});

  /// Moves the caret left by [count] graphemes after a paste.
  Future<bool> moveCaretLeft(int count);

  Future<bool> accessibilityGranted();

  /// Whether a password field has focus, or `null` when the platform cannot
  /// tell. Callers query it once per presentation, never by polling.
  Future<bool?> secureFieldFocused();
}

mixin _ForegroundPasteChannel implements ForegroundPasteHost {
  MethodChannel get _channel;

  @override
  Future<PasteOutcome> pasteForeground({bool plainText = false}) async {
    try {
      final name = await _channel.invokeMethod<Object?>('pasteForeground', {
        'plainText': plainText,
      });
      return PasteOutcome.values.firstWhere(
        (outcome) => outcome.name == name,
        orElse: () => PasteOutcome.unsupported,
      );
    } on MissingPluginException {
      return PasteOutcome.unsupported;
    } on PlatformException {
      return PasteOutcome.unsupported;
    }
  }

  @override
  Future<bool> moveCaretLeft(int count) async {
    if (count < 0 || count > ForegroundPasteHost.maxCaretMove) {
      throw RangeError.range(count, 0, ForegroundPasteHost.maxCaretMove);
    }
    if (count == 0) return true;
    try {
      return await _channel.invokeMethod<Object?>('moveCaretLeft', {
            'count': count,
          }) ==
          true;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  @override
  Future<bool?> secureFieldFocused() async {
    try {
      final focused = await _channel.invokeMethod<Object?>(
        'secureFieldFocused',
      );
      return focused is bool ? focused : null;
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    }
  }
}

abstract interface class QuickPasteWindowHost {
  Future<bool> isSupported();

  Future<void> prepare();

  Future<void> open();

  Future<bool> accessibilityGranted();

  Future<bool> requestAccessibility();

  void setOpenSettingsHandler(VoidCallback? handler);

  Future<void> dispose();
}

class MethodChannelQuickPasteWindowHost
    with _ForegroundPasteChannel
    implements QuickPasteWindowHost {
  MethodChannelQuickPasteWindowHost({MethodChannel? channel})
    : _channel =
          channel ?? const MethodChannel('com.copypaste.app/quick_paste_host') {
    _channel.setMethodCallHandler(_handleMethodCall);
  }

  @override
  final MethodChannel _channel;
  VoidCallback? _openSettings;
  VoidCallback? _openMain;
  VoidCallback? _findInHistory;

  void setFindInHistoryHandler(VoidCallback? handler) =>
      _findInHistory = handler;

  void setOpenMainHandler(VoidCallback? handler) => _openMain = handler;

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
    if (call.method == 'findInHistory') {
      _findInHistory?.call();
      return true;
    }
    if (call.method == 'openMain') {
      _openMain?.call();
      return true;
    }
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
    _openMain = null;
    _findInHistory = null;
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

/// [targetApp] is the receiving application's identifier (macOS bundle ID,
/// lowercase Windows image name, X11 window class), or null when unknown.
typedef QuickPasteOpenedHandler =
    Future<void> Function(
      int presentationId, {
      required bool inspectorVisible,
      String? targetApp,
    });

abstract interface class QuickPasteContextHost {
  Future<void> signalReady();

  Future<bool> accessibilityGranted();

  Future<bool> requestAccessibility();

  Future<void> setInspectorVisible({
    required int presentationId,
    required bool visible,
    bool remember = true,
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

abstract interface class QuickPasteFrameHost {
  Future<bool> presentFrame({required int presentationId});
}

class MethodChannelQuickPasteContextHost
    with _ForegroundPasteChannel
    implements QuickPasteContextHost, QuickPasteFrameHost {
  MethodChannelQuickPasteContextHost({MethodChannel? channel})
    : _channel =
          channel ??
          const MethodChannel('com.copypaste.app/quick_paste_context') {
    _channel.setMethodCallHandler(_handleMethodCall);
  }

  @override
  final MethodChannel _channel;
  QuickPasteOpenedHandler? _opened;
  Future<void> Function()? _shutdown;

  @override
  Future<bool> presentFrame({required int presentationId}) async {
    try {
      return await _channel.invokeMethod<Object?>('present', {
            'presentationId': presentationId,
          }) ==
          true;
    } on MissingPluginException {
      // Older and compositor-owned hosts already reveal their window.
      return true;
    } on PlatformException {
      return false;
    }
  }

  Future<String> runtimeEndpoint() async {
    final endpoint = await _channel.invokeMethod<Object?>('runtimeEndpoint');
    if (endpoint is! String ||
        endpoint.isEmpty ||
        !endpoint.startsWith('/') ||
        endpoint.contains('\u0000')) {
      throw PlatformException(code: 'runtime_endpoint_unavailable');
    }
    return endpoint;
  }

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
    bool remember = true,
  }) async {
    if (await _channel.invokeMethod<bool>('setInspectorVisible', {
          'presentationId': presentationId,
          'visible': visible,
          'remember': remember,
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
      final target = arguments['targetApp'];
      await _opened?.call(
        id,
        inspectorVisible: inspectorVisible,
        targetApp: _validTargetApp(target) ? target as String : null,
      );
      return true;
    }
    throw MissingPluginException(
      'Unsupported quick paste method: ${call.method}',
    );
  }

  static bool _validTargetApp(Object? value) =>
      value is String &&
      value.trim().isNotEmpty &&
      value.length <= 255 &&
      !value.runes.any((rune) => rune < 0x20 || rune == 0x7f);

  @override
  Future<void> dispose() async {
    _opened = null;
    _shutdown = null;
    _channel.setMethodCallHandler(null);
  }
}
