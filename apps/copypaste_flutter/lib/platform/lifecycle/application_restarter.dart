import 'package:flutter/services.dart';

/// Relaunches the native process after the owner has drained its runtime.
class ApplicationRestarter {
  const ApplicationRestarter();
  static const _channel = MethodChannel('com.copypaste.app/lifecycle');
  Future<void> restart() => _channel.invokeMethod<void>('restart');
}
