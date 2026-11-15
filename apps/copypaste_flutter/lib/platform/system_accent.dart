import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' show Color;

/// The accent color the operating system shows.
///
/// Channel contract: `read` answers an ARGB integer, or null where the
/// platform has no accent. The host sends `changed` with the same value when
/// the user picks another accent.
abstract interface class SystemAccentHost {
  Future<Color?> read();

  set onChanged(ValueChanged<Color?>? listener);
}

class MethodChannelSystemAccentHost implements SystemAccentHost {
  MethodChannelSystemAccentHost({
    MethodChannel channel = const MethodChannel(
      'com.copypaste.app/system_accent',
    ),
  }) : _channel = channel;

  final MethodChannel _channel;

  static Color? _decode(Object? value) => value is int ? Color(value) : null;

  @override
  Future<Color?> read() async {
    try {
      return _decode(await _channel.invokeMethod<Object?>('read'));
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    }
  }

  @override
  set onChanged(ValueChanged<Color?>? listener) {
    _channel.setMethodCallHandler(
      listener == null
          ? null
          : (call) async {
              if (call.method == 'changed') listener(_decode(call.arguments));
            },
    );
  }
}
