import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' show ThemeMode;

/// The system material behind a desktop window's translucent surfaces.
enum WindowMaterial {
  /// Opaque surfaces: the system has no material, the preference is off, or
  /// the system asks for reduced transparency.
  none,

  /// macOS 26 and later.
  liquidGlass,

  /// macOS before 26.
  vibrancy,

  /// Windows 11 main-window backdrop.
  mica,

  /// Windows 11 transient-window backdrop.
  acrylic,

  /// KWin blur behind an X11 window.
  blur;

  bool get translucent => this != none;
}

@immutable
class WindowMaterialState {
  const WindowMaterialState({
    this.material = WindowMaterial.none,
    this.supported = false,
    this.reducedTransparency = false,
  });

  final WindowMaterial material;

  /// The window can show a material whenever the preference and the system
  /// accessibility settings allow it.
  final bool supported;

  /// The system asks applications to keep surfaces opaque.
  final bool reducedTransparency;

  bool get translucent => material.translucent;

  static WindowMaterialState decode(Object? value) {
    if (value is! Map) return const WindowMaterialState();
    final name = value['material'];
    return WindowMaterialState(
      material: WindowMaterial.values.firstWhere(
        (material) => material.name == name,
        orElse: () => WindowMaterial.none,
      ),
      supported: value['supported'] == true,
      reducedTransparency: value['reducedTransparency'] == true,
    );
  }

  @override
  bool operator ==(Object other) =>
      other is WindowMaterialState &&
      other.material == material &&
      other.supported == supported &&
      other.reducedTransparency == reducedTransparency;

  @override
  int get hashCode => Object.hash(material, supported, reducedTransparency);
}

/// Native material of the window that hosts this Flutter engine.
///
/// Channel contract: `setEnabled {enabled: bool}` answers
/// `{material: String, supported: bool, reducedTransparency: bool}`, with
/// `material` a [WindowMaterial] name. The host sends `stateChanged` with the
/// same map when a system setting changes the material.
///
/// `setAppearance {mode: 'system' | 'light' | 'dark'}` makes the window's
/// system parts, such as its frame and material, follow the app theme.
abstract interface class WindowMaterialHost {
  Future<WindowMaterialState> setEnabled(bool enabled);

  Future<void> setAppearance(ThemeMode mode);

  set onStateChanged(ValueChanged<WindowMaterialState>? listener);
}

class MethodChannelWindowMaterialHost implements WindowMaterialHost {
  MethodChannelWindowMaterialHost({
    MethodChannel channel = const MethodChannel(
      'com.copypaste.app/window_material',
    ),
  }) : _channel = channel;

  final MethodChannel _channel;

  @override
  Future<WindowMaterialState> setEnabled(bool enabled) async {
    try {
      return WindowMaterialState.decode(
        await _channel.invokeMethod<Object?>('setEnabled', {
          'enabled': enabled,
        }),
      );
    } on MissingPluginException {
      return const WindowMaterialState();
    } on PlatformException {
      return const WindowMaterialState();
    }
  }

  @override
  Future<void> setAppearance(ThemeMode mode) async {
    try {
      await _channel.invokeMethod<void>('setAppearance', {'mode': mode.name});
    } on MissingPluginException {
      // The window keeps the system appearance.
    } on PlatformException {
      // The window keeps the system appearance.
    }
  }

  @override
  set onStateChanged(ValueChanged<WindowMaterialState>? listener) {
    _channel.setMethodCallHandler(
      listener == null
          ? null
          : (call) async {
              if (call.method == 'stateChanged') {
                listener(WindowMaterialState.decode(call.arguments));
              }
            },
    );
  }
}
