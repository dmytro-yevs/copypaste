import 'package:flutter/services.dart';

const _channel = MethodChannel('com.copypaste.app/tray_menu');

Future<void> bindMacosTrayQuickPaste(
  int nativeTrayAddress, {
  MethodChannel channel = _channel,
}) => channel.invokeMethod<void>('bindQuickPaste', {
  'nativeTrayAddress': nativeTrayAddress,
});

Future<void> unbindMacosTrayQuickPaste({MethodChannel channel = _channel}) =>
    channel.invokeMethod<void>('unbindQuickPaste');

/// Opts the nativeapi menu into displaying images on macOS 27 and later.
Future<void> showMacosTrayMenuImages(
  int nativeMenuAddress, {
  MethodChannel channel = _channel,
}) => channel.invokeMethod<void>('showImages', {
  'nativeMenuAddress': nativeMenuAddress,
});
