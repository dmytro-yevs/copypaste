import 'dart:async';
import 'dart:io';

import 'package:flutter/widgets.dart' show IconData, Offset, Rect, Size;
import 'package:screen_retriever/screen_retriever.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' show LucideIcons;
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

import 'desktop_window_controller.dart';
import 'tray_menu_icons.dart';
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
  Map<IconData, Image> _trayMenuImages = {};
  Menu? _trayMenu;
  MenuItem? _openMenuItem;
  MenuItem? _captureMenuItem;
  MenuItem? _settingsMenuItem;
  MenuItem? _quitMenuItem;
  ListenerId? _trayListener;
  ListenerId? _openMenuItemListener;
  ListenerId? _captureMenuItemListener;
  ListenerId? _settingsMenuItemListener;
  ListenerId? _quitMenuItemListener;

  @override
  Future<void> initializeWindow(DesktopWindowBounds initialBounds) async {
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
          title: 'CopyPaste',
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
    required Future<void> Function() onOpenRequested,
    required Future<void> Function() onCaptureRequested,
    required Future<void> Function() onSettingsRequested,
    required Future<void> Function() onQuitRequested,
  }) async {
    _disposeTray();
    final image = ImageAsset.fromAsset(_trayIconAsset);
    final icon = TrayIcon.create();
    final menu = Menu.create();
    final open = MenuItem.createWithLabelAndType('Open', MenuItemType.normal);
    final capture = MenuItem.createWithLabelAndType(
      'Pause capture',
      MenuItemType.normal,
    );
    final settings = MenuItem.createWithLabelAndType(
      'Settings',
      MenuItemType.normal,
    );
    final quit = MenuItem.createWithLabelAndType('Quit', MenuItemType.normal);

    if (image == null ||
        icon == null ||
        menu == null ||
        open == null ||
        capture == null ||
        settings == null ||
        quit == null) {
      image?.dispose();
      icon?.dispose();
      menu?.dispose();
      open?.dispose();
      capture?.dispose();
      settings?.dispose();
      quit?.dispose();
      throw StateError('Unable to create the system tray controls.');
    }

    ListenerId? openListener;
    ListenerId? captureListener;
    ListenerId? settingsListener;
    ListenerId? quitListener;
    ListenerId? trayListener;
    final menuImages = <IconData, Image>{};

    try {
      for (final glyph in const [
        LucideIcons.appWindow,
        LucideIcons.pause,
        LucideIcons.play,
        LucideIcons.settings,
        LucideIcons.power,
      ]) {
        menuImages[glyph] = await createTrayMenuIcon(glyph);
      }
      menu
        ..addItem(open)
        ..addItem(capture)
        ..addItem(settings)
        ..addSeparator()
        ..addItem(quit);
      open.icon = menuImages[LucideIcons.appWindow];
      capture.icon = menuImages[LucideIcons.pause];
      settings.icon = menuImages[LucideIcons.settings];
      quit.icon = menuImages[LucideIcons.power];
      if (Platform.isMacOS) {
        await showMacosTrayMenuImages(menu.nativeObject.address);
      }
      icon
        ..icon = image
        ..setTooltip('CopyPaste')
        ..setContextMenu(menu)
        ..setContextMenuTrigger(
          Platform.isMacOS
              ? ContextMenuTrigger.clicked
              : ContextMenuTrigger.rightClicked,
        );
      if (!icon.setVisible(true)) {
        throw StateError('Unable to show the system tray controls.');
      }
      openListener = open.addListener((event) {
        if (event is MenuItemClickedEvent) {
          unawaited(onOpenRequested());
        }
      });
      capture.isEnabled = false;
      captureListener = capture.addListener((event) {
        if (event is MenuItemClickedEvent) {
          unawaited(onCaptureRequested());
        }
      });
      settingsListener = settings.addListener((event) {
        if (event is MenuItemClickedEvent) {
          unawaited(onSettingsRequested());
        }
      });
      quitListener = quit.addListener((event) {
        if (event is MenuItemClickedEvent) {
          unawaited(onQuitRequested());
        }
      });
      trayListener = icon.addListener((event) {
        if ((Platform.isWindows || Platform.isLinux) &&
            (event is TrayIconClickedEvent ||
                event is TrayIconDoubleClickedEvent)) {
          unawaited(onOpenRequested());
        }
      });

      _trayIcon = icon;
      _trayImage = image;
      _trayMenuImages = menuImages;
      _trayMenu = menu;
      _openMenuItem = open;
      _captureMenuItem = capture;
      _settingsMenuItem = settings;
      _quitMenuItem = quit;
      _trayListener = trayListener;
      _openMenuItemListener = openListener;
      _captureMenuItemListener = captureListener;
      _settingsMenuItemListener = settingsListener;
      _quitMenuItemListener = quitListener;
    } catch (_) {
      _disposeTrayResources(
        icon: icon,
        image: image,
        menuImages: menuImages.values,
        menu: menu,
        open: open,
        capture: capture,
        settings: settings,
        quit: quit,
        trayListener: trayListener,
        openListener: openListener,
        captureListener: captureListener,
        settingsListener: settingsListener,
        quitListener: quitListener,
      );
      rethrow;
    }
  }

  @override
  Future<void> updateCaptureMenu({
    required bool available,
    required bool paused,
  }) async {
    final item = _captureMenuItem;
    if (item == null) return;
    item
      ..label = paused ? 'Resume capture' : 'Pause capture'
      ..icon = _trayMenuImages[paused ? LucideIcons.play : LucideIcons.pause]
      ..isEnabled = available;
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
    final image = _trayImage;
    final menuImages = _trayMenuImages;
    final menu = _trayMenu;
    final open = _openMenuItem;
    final capture = _captureMenuItem;
    final settings = _settingsMenuItem;
    final quit = _quitMenuItem;
    final trayListener = _trayListener;
    final openListener = _openMenuItemListener;
    final captureListener = _captureMenuItemListener;
    final settingsListener = _settingsMenuItemListener;
    final quitListener = _quitMenuItemListener;
    _trayIcon = null;
    _trayImage = null;
    _trayMenuImages = {};
    _trayMenu = null;
    _openMenuItem = null;
    _captureMenuItem = null;
    _settingsMenuItem = null;
    _quitMenuItem = null;
    _trayListener = null;
    _openMenuItemListener = null;
    _captureMenuItemListener = null;
    _settingsMenuItemListener = null;
    _quitMenuItemListener = null;
    _disposeTrayResources(
      icon: icon,
      image: image,
      menuImages: menuImages.values,
      menu: menu,
      open: open,
      capture: capture,
      settings: settings,
      quit: quit,
      trayListener: trayListener,
      openListener: openListener,
      captureListener: captureListener,
      settingsListener: settingsListener,
      quitListener: quitListener,
    );
  }

  void _disposeTrayResources({
    required TrayIcon? icon,
    required Image? image,
    required Iterable<Image> menuImages,
    required Menu? menu,
    required MenuItem? open,
    required MenuItem? capture,
    required MenuItem? settings,
    required MenuItem? quit,
    required ListenerId? trayListener,
    required ListenerId? openListener,
    required ListenerId? captureListener,
    required ListenerId? settingsListener,
    required ListenerId? quitListener,
  }) {
    if (icon != null && trayListener != null) {
      icon.removeListener(trayListener);
    }
    if (open != null && openListener != null) {
      open.removeListener(openListener);
    }
    if (capture != null && captureListener != null) {
      capture.removeListener(captureListener);
    }
    if (settings != null && settingsListener != null) {
      settings.removeListener(settingsListener);
    }
    if (quit != null && quitListener != null) {
      quit.removeListener(quitListener);
    }
    icon
      ?..setContextMenu(null)
      ..setVisible(false)
      ..dispose();
    open?.dispose();
    capture?.dispose();
    settings?.dispose();
    quit?.dispose();
    menu?.dispose();
    image?.dispose();
    for (final menuImage in menuImages) {
      menuImage.dispose();
    }
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
