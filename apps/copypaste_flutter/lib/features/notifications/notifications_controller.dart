import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../platform/notifications/pairing_notification_port.dart';
import '../devices/devices_controller.dart';
import '../devices/devices_gateway.dart';

class AppNotification {
  const AppNotification({
    required this.id,
    required this.peerName,
    required this.state,
    required this.incoming,
    this.read = false,
  });
  final String id;
  final String peerName;
  final PairingState state;
  final bool incoming;
  final bool read;
  bool get actionable => state == PairingState.awaitingConfirmation;
  String get title => switch (state) {
    PairingState.awaitingConfirmation =>
      incoming ? 'Connection request' : 'Confirm connection',
    PairingState.confirmed => 'Device connected',
    PairingState.rejected => 'Connection rejected',
    PairingState.cancelled => 'Connection cancelled',
    PairingState.timedOut => 'Connection request expired',
    _ => 'Connection failed',
  };
  AppNotification markRead() => AppNotification(
    id: id,
    peerName: peerName,
    state: state,
    incoming: incoming,
    read: true,
  );
}

/// Bounded session inbox. Stores display metadata only, never QR, codes or keys.
class NotificationsController extends ChangeNotifier {
  NotificationsController({
    required DevicesController devices,
    required PairingNotificationPort platform,
    required ValueListenable<bool> visible,
    required VoidCallback openPairing,
  }) : _devices = devices,
       _platform = platform,
       _visible = visible,
       _openPairing = openPairing {
    devices.addListener(_changed);
    _opened = platform.opened.listen(open);
    scheduleMicrotask(() {
      if (!_disposed) _changed();
    });
  }
  static const maxEntries = 50;
  final DevicesController _devices;
  final PairingNotificationPort _platform;
  final ValueListenable<bool> _visible;
  final VoidCallback _openPairing;
  StreamSubscription<String>? _opened;
  final List<AppNotification> _items = [];
  List<AppNotification> get items => List.unmodifiable(_items);
  int get unread => _items.where((item) => !item.read).length;
  bool _disposed = false;
  String? _shownRequest;
  String? _deliveryError;
  String? get deliveryError => _deliveryError;
  Future<void> enableSystemNotifications() async {
    try {
      final allowed = await _platform.requestPermission();
      if (_disposed) return;
      _deliveryError = allowed ? null : 'Allow system notifications in your OS settings to receive requests in the background.';
    } catch (_) {
      if (_disposed) return;
      _deliveryError = 'System notifications are unavailable. Connection requests remain here.';
    }
    notifyListeners();
  }

  void _changed() {
    final ceremony = _devices.pairing;
    final id = ceremony?.id;
    for (var i = 0; i < _items.length; i++) {
      final item = _items[i];
      if (item.actionable && item.id != id) {
        _items[i] = AppNotification(
          id: item.id,
          peerName: item.peerName,
          state: PairingState.cancelled,
          incoming: item.incoming,
        );
        if (_shownRequest == item.id) {
          _shownRequest = null;
          unawaited(_platform.dismiss().catchError((Object _) {}));
        }
        notifyListeners();
      }
    }
    if (ceremony == null || id == null) return;
    final index = _items.indexWhere((item) => item.id == id);
    if (index < 0 && ceremony.state != PairingState.awaitingConfirmation) {
      return;
    }
    if (index >= 0 && _items[index].state == ceremony.state) return;
    final item = AppNotification(
      id: id,
      peerName: ceremony.peerName ?? 'Nearby device',
      state: ceremony.state,
      incoming: _devices.incomingPairing,
    );
    if (index >= 0) _items.removeAt(index);
    _items.insert(0, item);
    if (_items.length > maxEntries) _items.removeLast();
    if (item.actionable && item.incoming && _shownRequest != id) {
      _shownRequest = id;
      if (_visible.value) {
        _openPairing();
      } else {
        unawaited(_deliver(item));
      }
    } else if (!item.actionable && _shownRequest == id) {
      _shownRequest = null;
      unawaited(_platform.dismiss().catchError((Object _) {}));
    }
    notifyListeners();
  }

  Future<void> _deliver(AppNotification item) async {
    try {
      await _platform.showRequest(requestId: item.id, peerName: item.peerName);
      // A request may have expired while the OS was initializing.
      if (_disposed || _shownRequest != item.id) await _platform.dismiss();
    } catch (_) {
      if (_disposed) return;
      _deliveryError = 'System notifications are unavailable. Connection requests remain here.';
      notifyListeners();
    }
  }

  bool canOpen(AppNotification item) =>
      item.actionable &&
      _devices.pairing?.id == item.id &&
      _devices.pairing?.state == PairingState.awaitingConfirmation;

  void open(String id) {
    if (_disposed) return;
    final index = _items.indexWhere((item) => item.id == id);
    if (index < 0 || !canOpen(_items[index])) return;
    _items[index] = _items[index].markRead();
    _openPairing();
    notifyListeners();
  }

  void markAllRead() {
    for (var i = 0; i < _items.length; i++) {
      _items[i] = _items[i].markRead();
    }
    notifyListeners();
  }

  void clearRead() {
    _items.removeWhere((item) => item.read && !item.actionable);
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _devices.removeListener(_changed);
    unawaited(_opened?.cancel());
    if (_shownRequest != null) {
      unawaited(_platform.dismiss().catchError((Object _) {}));
    }
    _items.clear();
    super.dispose();
  }
}
