import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

import '../features/content_rules/controller/content_rules_controller.dart';
import '../features/content_rules/view/content_rules_search_targets.dart';
import '../features/paste_queue/controller/paste_queue_controller.dart';
import '../features/paste_queue/controller/paste_queue_shortcuts_controller.dart';
import '../features/paste_queue/view/paste_queue_header_action.dart';
import '../features/paste_queue/view/paste_queue_settings_view.dart';
import '../features/quick_access/controller/quick_access_controller.dart';
import '../features/quick_access/view/quick_access_settings_section.dart';
import '../features/settings/controller/settings_navigation_state.dart';
import '../features/settings/view/settings_search_target.dart';
import '../features/snippets/controller/snippets_controller.dart';
import '../features/snippets/view/snippets_search_targets.dart';
import '../features/transforms/controller/transforms_settings_controller.dart';
import '../features/transforms/view/transforms_settings_view.dart';
import '../features/tray/controller/tray_menu_controller.dart';
import '../features/tray/view/tray_settings_view.dart';
import '../platform/desktop/global_shortcut.dart';

/// The main window's feature controllers that outlive any one destination:
/// the paste queue, the settings-backed features and the tray's feature rows. They are created with the runtime connection and
/// released with it; each loads its data only while a view needs it.
class AppFeatures {
  AppFeatures({
    required this.pasteQueue,
    required this.snippets,
    required this.transforms,
    required this.contentRules,
    required this.openSettings,
    this.pasteQueueShortcuts,
    this.quickAccess,
    this.tray,
    bool Function()? capturePaused,
  }) {
    final queueSettings = PasteQueueSettingsContribution(
      pasteQueue,
      shortcuts: pasteQueueShortcuts,
    );
    pasteQueueSettingsSection = queueSettings.section;
    // Contributions own the GlobalKeys settings search scrolls to, so they
    // are built once rather than on every rebuild of the app.
    settingsSections = List.unmodifiable([
      if (tray case final tray?) TraySettingsContribution(tray).section,
      contentRulesSettingsSection(
        contentRules,
        onOpenPrivacy: () => openSettings(SettingsSectionId.privacy),
      ),
      queueSettings.feature,
      if (quickAccess case final quickAccess?)
        quickAccessSettingsSection(
          controller: quickAccess,
          capturePaused: capturePaused ?? () => false,
        ),
      snippetsSettingsSection(snippets),
      transformsSettingsSection(transforms),
    ]);
  }

  final PasteQueueController pasteQueue;
  final PasteQueueShortcutsController? pasteQueueShortcuts;
  final SnippetsController snippets;
  final TransformsSettingsController transforms;
  final ContentRulesController contentRules;

  /// Android only.
  final QuickAccessController? quickAccess;

  /// Desktops with a tray or menu-bar menu only.
  final TrayMenuController? tray;

  /// Shows Settings at the given section.
  final ValueChanged<SettingsSectionId> openSettings;

  /// Where the paste queue settings live on this platform.
  late final SettingsSectionId pasteQueueSettingsSection;

  late final List<SettingsFeatureSection> settingsSections;

  /// The header control, which also opens the queue drawer for the tray.
  final pasteQueueAction = GlobalKey<PasteQueueHeaderActionState>(
    debugLabel: 'paste-queue-action',
  );

  String? _quickPasteShortcut;
  bool _started = false;
  bool _disposed = false;

  /// Work that must finish before the presentation may be released.
  bool get busy =>
      pasteQueue.deliveryInFlight ||
      (pasteQueueShortcuts?.busy ?? false) ||
      snippets.busy ||
      transforms.busy ||
      contentRules.busy ||
      (quickAccess?.busy ?? false);

  /// Loads the queue, registers its shortcuts and feeds the tray's rows.
  Future<void> start() async {
    if (_started || _disposed) return;
    _started = true;
    await pasteQueue.start();
    if (_disposed) return;
    final tray = this.tray;
    if (tray != null) {
      pasteQueue.trayStatus.addListener(_syncTrayQueue);
      await Future.wait([
        tray.start(),
        tray.setNewSnippetHandler(_newSnippet),
        tray.setPasteQueueHandler(openPasteQueue),
        tray.setContentRulesHandler(
          () => openSettings(SettingsSectionId.rules),
        ),
        tray.setPasteQueue(pasteQueue.trayStatus.value),
      ]);
    }
    if (_disposed) return;
    await pasteQueueShortcuts?.start();
  }

  /// Opens the queue drawer; a presentation released while hidden is
  /// rebuilt first, so the drawer opens after that frame.
  void openPasteQueue() {
    final action = pasteQueueAction.currentState;
    if (action != null) {
      unawaited(action.open());
      return;
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!_disposed) unawaited(pasteQueueAction.currentState?.open());
    });
  }

  /// Shows the Quick Paste shortcut on the tray's Open Quick Paste item.
  /// [description] is the trigger a Linux portal confirmed, when it did.
  void showQuickPasteShortcut(
    DesktopShortcut? shortcut, {
    String? description,
  }) {
    final text =
        description ??
        (shortcut == null ? null : desktopShortcutText(shortcut));
    if (_disposed || text == _quickPasteShortcut) return;
    _quickPasteShortcut = text;
    unawaited(tray?.setQuickPasteShortcut(text).catchError((Object _) {}));
  }

  void _newSnippet() {
    snippets.requestNewSnippet();
    openSettings(SettingsSectionId.snippets);
  }

  void _syncTrayQueue() {
    if (_disposed) return;
    unawaited(
      tray
          ?.setPasteQueue(pasteQueue.trayStatus.value)
          .catchError((Object _) {}),
    );
  }

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    pasteQueue.trayStatus.removeListener(_syncTrayQueue);
    pasteQueueShortcuts?.dispose();
    quickAccess?.dispose();
    snippets.dispose();
    transforms.dispose();
    contentRules.dispose();
    final tray = this.tray;
    try {
      if (tray != null) {
        // The menu outlives a runtime restart; its rows must not point here.
        await Future.wait([
          tray.setNewSnippetHandler(null),
          tray.setPasteQueueHandler(null),
          tray.setContentRulesHandler(null),
          tray.setPasteQueue(null),
          tray.setQuickPasteShortcut(null),
        ]);
        await tray.dispose();
      }
    } finally {
      pasteQueue.dispose();
    }
  }
}

/// [shortcut] as the platform writes it: ⌃⌥⇧⌘V on macOS, Ctrl+Alt+Shift+V
/// on Windows and Linux.
String desktopShortcutText(
  DesktopShortcut shortcut, {
  TargetPlatform? platform,
}) {
  final macOS = (platform ?? defaultTargetPlatform) == TargetPlatform.macOS;
  final windows = (platform ?? defaultTargetPlatform) == TargetPlatform.windows;
  final modifiers = [
    for (final modifier in _modifierOrder)
      if (shortcut.modifiers.contains(modifier))
        switch (modifier) {
          DesktopShortcutModifier.control => macOS ? '⌃' : 'Ctrl',
          DesktopShortcutModifier.alt => macOS ? '⌥' : 'Alt',
          DesktopShortcutModifier.shift => macOS ? '⇧' : 'Shift',
          DesktopShortcutModifier.meta =>
            macOS
                ? '⌘'
                : windows
                ? 'Win'
                : 'Super',
        },
  ];
  final label = shortcut.displayKeys.last.keyLabel.trim();
  final key = label.isEmpty ? shortcut.key.debugName ?? '' : label;
  return macOS ? '${modifiers.join()}$key' : [...modifiers, key].join('+');
}

const _modifierOrder = [
  DesktopShortcutModifier.control,
  DesktopShortcutModifier.alt,
  DesktopShortcutModifier.shift,
  DesktopShortcutModifier.meta,
];
