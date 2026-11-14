import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../../features/history/models/history_models.dart';
import '../../generated/api/applications.dart' as bridge;

/// An application installed on this device, by the identifier capture
/// attribution records for it: a macOS bundle ID, a lowercase Windows image
/// name, a Linux desktop entry ID or an Android package name.
@immutable
class InstalledApplication {
  const InstalledApplication({required this.id, required this.name});

  final String id;
  final String name;
}

/// Applications installed on this device. Each call loads only what it
/// answers; nothing is retained between calls.
abstract interface class InstalledApplications {
  factory InstalledApplications.forPlatform() =>
      defaultTargetPlatform == TargetPlatform.android
      ? const AndroidInstalledApplications()
      : const DesktopInstalledApplications();

  Future<List<InstalledApplication>> list();

  Future<InstalledApplication?> find(String id);

  Future<HistorySourceAppIcon?> icon(String id);
}

/// macOS, Windows and Linux through the native bridge.
class DesktopInstalledApplications implements InstalledApplications {
  const DesktopInstalledApplications();

  @override
  Future<List<InstalledApplication>> list() async => [
    for (final app in await bridge.installedApplications())
      InstalledApplication(id: app.id, name: app.name),
  ];

  @override
  Future<InstalledApplication?> find(String id) async {
    final app = await bridge.installedApplication(id: id);
    return app == null
        ? null
        : InstalledApplication(id: app.id, name: app.name);
  }

  @override
  Future<HistorySourceAppIcon?> icon(String id) async {
    final icon = await bridge.installedApplicationIcon(id: id);
    return icon == null
        ? null
        : HistorySourceAppIcon(
            icon.pngBytes,
            width: icon.width,
            height: icon.height,
          );
  }
}

/// Launcher apps through the Android host.
class AndroidInstalledApplications implements InstalledApplications {
  const AndroidInstalledApplications({
    MethodChannel channel = const MethodChannel(
      'com.copypaste.app/installed_apps',
    ),
  }) : _channel = channel;

  final MethodChannel _channel;

  @override
  Future<List<InstalledApplication>> list() async {
    final apps = await _channel.invokeListMethod<Map<Object?, Object?>>('list');
    return [for (final app in apps ?? const []) ?_application(app)];
  }

  @override
  Future<InstalledApplication?> find(String id) async => _application(
    await _channel.invokeMapMethod<Object?, Object?>('find', {'id': id}),
  );

  @override
  Future<HistorySourceAppIcon?> icon(String id) async {
    final icon = await _channel.invokeMapMethod<Object?, Object?>('icon', {
      'id': id,
    });
    return switch (icon) {
      {'bytes': final Uint8List bytes, 'width': final int width} =>
        HistorySourceAppIcon(
          bytes,
          width: width,
          height: icon['height'] as int?,
        ),
      _ => null,
    };
  }

  static InstalledApplication? _application(Map<Object?, Object?>? value) =>
      switch (value) {
        {'id': final String id, 'name': final String name} =>
          InstalledApplication(id: id, name: name),
        _ => null,
      };
}
