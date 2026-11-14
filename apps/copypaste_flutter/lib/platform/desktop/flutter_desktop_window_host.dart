import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart' show listEquals;
import 'package:flutter/widgets.dart' show IconData, Offset, Rect, Size;
import 'package:flutter/services.dart'
    show MissingPluginException, PlatformException;
import 'package:screen_retriever/screen_retriever.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

import 'desktop_window_controller.dart';
import 'tray_menu_icons.dart';
import 'quick_paste_host.dart';
import '../macos/macos_tray_menu.dart';

const _trayIconAsset = 'assets/brand/copypaste.png';

class FlutterDesktopWindowHost
    with WindowListener
    implements DesktopWindowHost {
  Future<void> Function()? _onCloseRequested;
  Future<void> Function()? _onBoundsChanged;
  void Function(bool)? _onVisibilityChanged;
  bool _listening = false;
  bool _preventsClose = false;
  bool _reuseExistingGeometry = false;

  TrayIcon? _trayIcon;
  Image? _trayImage;
  ListenerId? _trayListener;
  _NativeMenu? _trayMenu;
  TrayMenuEvents? _trayEvents;
  final Map<String, _NativeItem> _trayItems = {};
  final Map<IconData, Image> _trayGlyphs = {};
  Future<void> _trayRender = Future<void>.value();
  bool _openingQuickPaste = false;
  bool _nativeTrayActivationBound = false;

  Future<void> _unbindNativeTrayActivation() async {
    if (!_nativeTrayActivationBound) return;
    try {
      await unbindMacosTrayQuickPaste();
    } on MissingPluginException {
      // Native teardown may have already released its channel.
    } on PlatformException {
      // Releasing the remaining tray resources must still complete.
    } finally {
      _nativeTrayActivationBound = false;
    }
  }

  Future<void> _openQuickPasteFromTray(TrayIcon icon) async {
    if (_openingQuickPaste) return;
    _openingQuickPaste = true;
    try {
      await openTrayQuickPaste(icon.getBounds());
    } catch (_) {
      if (identical(icon, _trayIcon)) icon.openContextMenu();
    } finally {
      _openingQuickPaste = false;
    }
  }

  @override
  Future<void> initializeWindow(
    DesktopWindowBounds initialBounds, {
    bool showInTaskbar = true,
  }) async {
    await windowManager.ensureInitialized();
    _reuseExistingGeometry = await windowManager.isPreventClose();
    if (!_listening) {
      windowManager.addListener(this);
      _listening = true;
    }
    if (!_reuseExistingGeometry) {
      await windowManager.waitUntilReadyToShow(
        WindowOptions(
          size: Size(initialBounds.width, initialBounds.height),
          // macOS owns the bundle-specific title, including Dev identity.
          title: Platform.isMacOS ? null : 'CopyPaste',
          skipTaskbar: !showInTaskbar,
        ),
      );
    }
    if (Platform.isMacOS) {
      try {
        await windowManager.setTitleBarStyle(
          TitleBarStyle.hidden,
          windowButtonVisibility: true,
        );
      } catch (_) {
        try {
          await windowManager.setTitleBarStyle(
            TitleBarStyle.normal,
            windowButtonVisibility: true,
          );
        } catch (_) {
          // Preserve the original title bar configuration error.
        }
        rethrow;
      }
    }
    await setTaskbarVisible(showInTaskbar);
    await windowManager.show();
    await windowManager.focus();
  }

  @override
  Future<void> setCloseHandler(Future<void> Function() onCloseRequested) async {
    _onCloseRequested = onCloseRequested;
    await windowManager.setPreventClose(true);
    _preventsClose = true;
  }

  @override
  Future<DesktopWorkArea> workAreaFor(DesktopWindowBounds bounds) async {
    final displays = await screenRetriever.getAllDisplays();
    final display = _displayWithGreatestOverlap(displays, bounds);
    final position = display.visiblePosition ?? Offset.zero;
    final size = display.visibleSize ?? display.size;
    return DesktopWorkArea(
      left: position.dx,
      top: position.dy,
      width: size.width,
      height: size.height,
    );
  }

  @override
  Future<void> setBounds(DesktopWindowBounds bounds) {
    if (_reuseExistingGeometry) {
      _reuseExistingGeometry = false;
      return Future<void>.value();
    }
    return windowManager.setBounds(
      Rect.fromLTWH(bounds.left, bounds.top, bounds.width, bounds.height),
    );
  }

  @override
  Future<void> setMinimumSize(DesktopWindowSize size) {
    if (_reuseExistingGeometry) {
      return Future<void>.value();
    }
    return windowManager.setMinimumSize(Size(size.width, size.height));
  }

  @override
  Future<DesktopWindowBounds> getBounds() async {
    final bounds = await windowManager.getBounds();
    return DesktopWindowBounds(
      left: bounds.left,
      top: bounds.top,
      width: bounds.width,
      height: bounds.height,
    );
  }

  @override
  Future<void> initializeTray({
    bool visible = true,
    required List<TrayMenuEntry> menu,
    required TrayMenuEvents events,
  }) async {
    await _unbindNativeTrayActivation();
    _disposeTray();
    final image = ImageAsset.fromAsset(_trayIconAsset);
    final icon = TrayIcon.create();
    final root = Menu.create();
    if (image == null || icon == null || root == null) {
      image?.dispose();
      icon?.dispose();
      root?.dispose();
      throw StateError('Unable to create the system tray controls.');
    }
    _trayIcon = icon;
    _trayImage = image;
    _trayEvents = events;
    final native = _trayMenu = _NativeMenu(root);
    try {
      native.listener = root.addListener((event) {
        final events = _trayEvents;
        if (events == null) return;
        if (event is MenuOpenedEvent && event.menuId == root.id) {
          events.onOpened();
        } else if (event is MenuClosedEvent && event.menuId == root.id) {
          events.onClosed();
        }
      });
      await _showTrayMenu(native, menu);
      icon
        ..icon = image
        ..setTitle(Platform.isMacOS ? null : 'CopyPaste')
        ..setTooltip('CopyPaste')
        ..setContextMenu(root)
        ..setContextMenuTrigger(
          // Linux exports its D-Bus menu only with the primary menu trigger.
          Platform.isLinux
              ? ContextMenuTrigger.clicked
              : ContextMenuTrigger.rightClicked,
        );
      if (!icon.setVisible(visible)) {
        throw StateError('Unable to show the system tray controls.');
      }
      _trayListener = icon.addListener((event) {
        if (Platform.isLinux && event is TrayIconClickedEvent) {
          final events = _trayEvents;
          if (events != null) {
            unawaited(
              events.onAction(const TrayCommandAction(TrayCommand.openApp)),
            );
          }
        } else if (event is TrayIconClickedEvent &&
            !_nativeTrayActivationBound) {
          unawaited(_openQuickPasteFromTray(icon));
        }
      });
      if (Platform.isMacOS) {
        await bindMacosTrayQuickPaste(icon.nativeObject.address);
        _nativeTrayActivationBound = true;
      }
    } catch (_) {
      _disposeTray();
      rethrow;
    }
  }

  @override
  Future<void> updateTrayMenu(List<TrayMenuEntry> menu) {
    _trayRender = _trayRender.catchError((Object _) {}).then((_) async {
      final native = _trayMenu;
      final icon = _trayIcon;
      if (native == null || icon == null) return;
      final changed = await _showTrayMenu(native, menu);
      // A D-Bus menu host re-reads the layout only when it is told to.
      if (changed && Platform.isLinux && identical(native, _trayMenu)) {
        icon.setContextMenu(native.menu);
      }
    });
    return _trayRender;
  }

  @override
  Future<void> openQuickPaste() async {
    final icon = _trayIcon;
    if (icon != null) await _openQuickPasteFromTray(icon);
  }

  /// Renders the glyphs [menu] needs, then shows it in [root]. Returns
  /// whether any native row changed.
  Future<bool> _showTrayMenu(_NativeMenu root, List<TrayMenuEntry> menu) async {
    for (final glyph in _glyphsOf(menu)) {
      if (!_trayGlyphs.containsKey(glyph)) {
        _trayGlyphs[glyph] = await createTrayMenuIcon(glyph);
      }
    }
    if (!identical(root, _trayMenu)) return false;
    final restructured = <Menu>[];
    final changed = _renderTrayMenu(root, menu, restructured);
    if (Platform.isMacOS) {
      for (final target in restructured) {
        await showMacosTrayMenuImages(target.nativeObject.address);
      }
    }
    return changed;
  }

  /// Reuses the native item of every key that persists, so a rebuild neither
  /// re-renders glyphs nor registers listeners again.
  bool _renderTrayMenu(
    _NativeMenu target,
    List<TrayMenuEntry> entries,
    List<Menu> restructured,
  ) {
    final keys = [for (final entry in entries) entry.key];
    final restructure = !listEquals(keys, target.keys);
    var changed = restructure;
    if (restructure) {
      target.menu.clear();
      restructured.add(target.menu);
    }
    for (final entry in entries) {
      if (entry is TrayMenuSeparator) {
        if (restructure) target.menu.addSeparator();
        continue;
      }
      final native = _trayItems[entry.key] ??= _createTrayItem(entry);
      changed = _applyTrayItem(native, entry) || changed;
      if (restructure) target.menu.addItem(native.item);
      if (entry is TrayMenuSubmenu) {
        changed =
            _renderTrayMenu(native.submenu!, entry.children, restructured) ||
            changed;
      }
    }
    target.keys = keys;
    return changed;
  }

  _NativeItem _createTrayItem(TrayMenuEntry entry) {
    final submenu = entry is TrayMenuSubmenu;
    final item = MenuItem.createWithLabelAndType(
      '',
      submenu ? MenuItemType.submenu : MenuItemType.normal,
    );
    final menu = submenu ? Menu.create() : null;
    if (item == null || (submenu && menu == null)) {
      item?.dispose();
      menu?.dispose();
      throw StateError('Unable to create a system tray menu item.');
    }
    late final _NativeItem native;
    final listener = item.addListener((event) {
      final spec = native.spec;
      if (event is MenuItemClickedEvent &&
          spec is TrayMenuItem &&
          spec.selectable) {
        final events = _trayEvents;
        if (events != null) unawaited(events.onAction(spec.action!));
      }
    });
    native = _NativeItem(
      item,
      listener,
      menu == null ? null : _NativeMenu(menu),
    );
    if (menu != null) item.submenu = menu;
    return native;
  }

  bool _applyTrayItem(_NativeItem native, TrayMenuEntry entry) {
    final previous = native.spec;
    native.spec = entry;
    final (label, icon, enabled) = switch (entry) {
      TrayMenuItem() => (entry.label, entry.icon, entry.selectable),
      TrayMenuSubmenu() => (entry.label, entry.icon, entry.enabled),
      TrayMenuSeparator() => ('', null, false),
    };
    final (oldLabel, oldIcon, oldEnabled) = switch (previous) {
      TrayMenuItem() => (previous.label, previous.icon, previous.selectable),
      TrayMenuSubmenu() => (previous.label, previous.icon, previous.enabled),
      _ => (null, null, null),
    };
    var changed = false;
    if (label != oldLabel) {
      // Windows reads a single ampersand as a mnemonic marker.
      native.item.label = Platform.isWindows
          ? label.replaceAll('&', '&&')
          : label;
      changed = true;
    }
    if (previous == null || icon != oldIcon) {
      native.item.icon = icon == null ? null : _trayGlyphs[icon];
      changed = true;
    }
    if (enabled != oldEnabled) {
      native.item.isEnabled = enabled;
      changed = true;
    }
    return changed;
  }

  static Iterable<IconData> _glyphsOf(List<TrayMenuEntry> entries) sync* {
    for (final entry in entries) {
      switch (entry) {
        case TrayMenuItem(:final icon?):
          yield icon;
        case TrayMenuSubmenu(:final icon, :final children):
          if (icon != null) yield icon;
          yield* _glyphsOf(children);
        default:
          break;
      }
    }
  }

  @override
  Future<void> setTaskbarVisible(bool visible) async {
    await windowManager.setSkipTaskbar(!visible);
    if (await windowManager.isSkipTaskbar() != !visible) {
      throw StateError('Unable to change Dock or taskbar visibility.');
    }
  }

  @override
  Future<void> setTrayVisible(bool visible) async {
    final icon = _trayIcon;
    if (icon == null || !icon.setVisible(visible)) {
      throw StateError('Unable to change system tray visibility.');
    }
  }

  @override
  Future<void> hide() => windowManager.hide();

  @override
  Future<void> showAndFocus() async {
    if (await windowManager.isMinimized()) await windowManager.restore();
    await windowManager.show();
    await windowManager.focus();
  }

  @override
  Future<void> quit() => windowManager.destroy();

  @override
  Future<bool> isFullScreen() => windowManager.isFullScreen();

  @override
  Future<void> setFullScreen(bool enabled) =>
      windowManager.setFullScreen(enabled);

  @override
  void setBoundsChangedHandler(Future<void> Function() onBoundsChanged) {
    _onBoundsChanged = onBoundsChanged;
  }

  @override
  void setVisibilityChangedHandler(void Function(bool)? onVisibilityChanged) {
    _onVisibilityChanged = onVisibilityChanged;
  }

  @override
  Future<void> dispose() async {
    _onCloseRequested = null;
    _onBoundsChanged = null;
    _onVisibilityChanged = null;
    await _unbindNativeTrayActivation();
    _disposeTray();
    if (_listening) {
      windowManager.removeListener(this);
      _listening = false;
    }
    if (_preventsClose) {
      _preventsClose = false;
      await windowManager.setPreventClose(false);
    }
  }

  @override
  void onWindowClose() {
    final onCloseRequested = _onCloseRequested;
    if (onCloseRequested != null) {
      unawaited(onCloseRequested());
    }
  }

  @override
  void onWindowFocus() {
    // Native opens, including the macOS Dock, can focus without a show event.
    _onVisibilityChanged?.call(true);
  }

  @override
  void onWindowMinimize() => _onVisibilityChanged?.call(false);

  @override
  void onWindowRestore() => _onVisibilityChanged?.call(true);

  @override
  void onWindowEvent(String eventName) {
    if (eventName == 'hide') _onVisibilityChanged?.call(false);
    if (eventName == 'show') _onVisibilityChanged?.call(true);
  }

  @override
  void onWindowMoved() {
    _saveBounds();
  }

  @override
  void onWindowResized() {
    _saveBounds();
  }

  void _saveBounds() {
    final onBoundsChanged = _onBoundsChanged;
    if (onBoundsChanged != null) {
      unawaited(onBoundsChanged());
    }
  }

  void _disposeTray() {
    final icon = _trayIcon;
    final listener = _trayListener;
    final image = _trayImage;
    final root = _trayMenu;
    _trayIcon = null;
    _trayListener = null;
    _trayImage = null;
    _trayMenu = null;
    _trayEvents = null;
    if (icon != null && listener != null) icon.removeListener(listener);
    icon
      ?..setContextMenu(null)
      ..setVisible(false)
      ..dispose();
    for (final item in _trayItems.values) {
      item.item.removeListener(item.listener);
      item.item.dispose();
      item.submenu?.menu.dispose();
    }
    _trayItems.clear();
    if (root != null) {
      if (root.listener case final listener?) {
        root.menu.removeListener(listener);
      }
      root.menu.dispose();
    }
    image?.dispose();
    for (final glyph in _trayGlyphs.values) {
      glyph.dispose();
    }
    _trayGlyphs.clear();
  }

  Display _displayWithGreatestOverlap(
    List<Display> displays,
    DesktopWindowBounds bounds,
  ) {
    var selected = displays.first;
    var greatestOverlap = -1.0;
    for (final display in displays) {
      final position = display.visiblePosition ?? Offset.zero;
      final size = display.visibleSize ?? display.size;
      final overlap = _overlapArea(
        bounds,
        DesktopWorkArea(
          left: position.dx,
          top: position.dy,
          width: size.width,
          height: size.height,
        ),
      );
      if (overlap > greatestOverlap) {
        selected = display;
        greatestOverlap = overlap;
      }
    }
    return selected;
  }
}

double _overlapArea(DesktopWindowBounds bounds, DesktopWorkArea workArea) {
  final horizontal =
      (bounds.left + bounds.width).clamp(workArea.left, workArea.right) -
      bounds.left.clamp(workArea.left, workArea.right);
  final vertical =
      (bounds.top + bounds.height).clamp(workArea.top, workArea.bottom) -
      bounds.top.clamp(workArea.top, workArea.bottom);
  return horizontal * vertical;
}

class _NativeMenu {
  _NativeMenu(this.menu);

  final Menu menu;
  ListenerId? listener;
  List<String> keys = const [];
}

class _NativeItem {
  _NativeItem(this.item, this.listener, this.submenu);

  final MenuItem item;
  final ListenerId listener;
  final _NativeMenu? submenu;
  TrayMenuEntry? spec;
}
