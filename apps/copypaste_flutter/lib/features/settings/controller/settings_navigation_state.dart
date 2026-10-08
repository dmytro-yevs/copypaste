import 'package:shadcn_flutter/shadcn_flutter.dart' show IconData, LucideIcons;

/// Lightweight settings navigation retained while presentation is unloaded.
class SettingsNavigationState {
  SettingsSectionId _section = SettingsSectionId.clipboard;
  String _searchText = '';
  String? _selectedTargetId;

  SettingsSectionId get section => _section;
  String get searchText => _searchText;
  String? get selectedTargetId => _selectedTargetId;

  void search(String value) {
    _searchText = value;
    _selectedTargetId = null;
  }

  void select(SettingsSectionId value, {String? targetId}) {
    _section = value;
    _selectedTargetId = targetId;
  }
}

enum SettingsSectionId {
  clipboard(
    label: 'Clipboard',
    slug: 'clipboard',
    description: 'Capture and history limits.',
    icon: LucideIcons.clipboard,
  ),
  privacy(
    label: 'Privacy',
    slug: 'privacy',
    description: 'Application exclusions and screen protection.',
    icon: LucideIcons.shield,
  ),
  quickPaste(
    label: 'Quick Paste',
    slug: 'quick-paste',
    description: 'Shortcut and automatic paste behavior.',
    icon: LucideIcons.keyboard,
  ),
  sync(
    label: 'Sync',
    slug: 'sync',
    description: 'Synchronization and nearby-device discovery.',
    icon: LucideIcons.refreshCw,
  ),
  notifications(
    label: 'Notifications',
    slug: 'notifications',
    description: 'Capture notifications and sounds.',
    icon: LucideIcons.bell,
  ),
  data(
    label: 'Data',
    slug: 'data',
    description: 'Export, backup, and restore.',
    icon: LucideIcons.database,
  ),
  modules(
    label: 'Modules',
    slug: 'modules',
    description: 'Install and manage optional modules.',
    icon: LucideIcons.puzzle,
  ),
  about(
    label: 'About',
    slug: 'about',
    description: 'Version and application updates.',
    icon: LucideIcons.info,
  );

  const SettingsSectionId({
    required this.label,
    required this.slug,
    required this.description,
    required this.icon,
  });

  final String label;
  final String slug;
  final String description;
  final IconData icon;
}
