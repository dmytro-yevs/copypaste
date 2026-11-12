import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../controller/settings_navigation_state.dart';

/// A settings destination that navigation lists and search can reveal.
class SettingsSearchTarget {
  const SettingsSearchTarget({
    required this.id,
    required this.section,
    required this.label,
    required this.description,
    required this.targetKey,
    this.keywords = '',
  });

  /// Search results use a `result-` prefix; section entries use `section-`.
  final String id;
  final SettingsSectionId section;
  final String label;
  final String description;
  final String keywords;

  /// The row scrolled into view when the target is selected.
  final GlobalKey targetKey;

  String get widgetKey => 'settings-$id';
  bool get isSearchResult => id.startsWith('result-');

  bool matches(String query) {
    final searchable = [
      section.label,
      label,
      description,
      keywords,
    ].join(' ').toLowerCase();
    return query
        .split(RegExp(r'\s+'))
        .where((token) => token.isNotEmpty)
        .every(searchable.contains);
  }
}

/// Presentation state a feature section needs from the settings screen.
class SettingsSectionScope {
  const SettingsSectionScope({required this.highlightedTargetId});

  final String? highlightedTargetId;

  bool isHighlighted(String targetId) => highlightedTargetId == targetId;
}

/// Feature-owned content of one settings section.
///
/// Several contributions to the same section render in list order, so a
/// feature can add rows to a section it does not own. A section with no
/// contribution, or unavailable on the current platform, is not listed.
class SettingsFeatureSection {
  const SettingsFeatureSection({
    required this.section,
    required this.builder,
    this.searchTargets = _noTargets,
  });

  final SettingsSectionId section;
  final Widget Function(BuildContext context, SettingsSectionScope scope)
  builder;

  /// Rows this contribution exposes to settings search.
  final List<SettingsSearchTarget> Function(BuildContext context) searchTargets;

  static List<SettingsSearchTarget> _noTargets(BuildContext context) =>
      const [];
}
