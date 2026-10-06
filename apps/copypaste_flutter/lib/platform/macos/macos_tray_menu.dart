import 'package:flutter/services.dart';

const _channel = MethodChannel('com.copypaste.app/tray_menu');

/// Opts the nativeapi menu into displaying images on macOS 27 and later.
Future<void> showMacosTrayMenuImages(
  int nativeMenuAddress, {
  MethodChannel channel = _channel,
}) => channel.invokeMethod<void>('showImages', {
  'nativeMenuAddress': nativeMenuAddress,
});
