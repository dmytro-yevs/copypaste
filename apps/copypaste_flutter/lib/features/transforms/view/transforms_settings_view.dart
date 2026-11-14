import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../applications/controller/application_directory.dart';
import '../../applications/repository/application_catalog.dart';
import '../../applications/view/application_picker.dart';
import '../../settings/controller/settings_navigation_state.dart';
import '../../settings/view/settings_search_target.dart';
import '../../settings/view/settings_section.dart';
import '../controller/transforms_settings_controller.dart';
import '../models/transform_models.dart';
import 'pipeline_editor_drawer.dart';

/// Settings → Transforms as a settings screen contribution.
SettingsFeatureSection transformsSettingsSection(
  TransformsSettingsController controller, {
  ApplicationCatalog applications = const RuntimeApplicationCatalog(),
}) {
  final keys = TransformsSettingsKeys();
  return SettingsFeatureSection(
    section: SettingsSectionId.transforms,
    builder: (context, scope) => TransformsSettingsView(
      controller: controller,
      keys: keys,
      scope: scope,
      applications: applications,
    ),
    searchTargets: (context) => keys.targets(context),
  );
}

/// Rows that settings search can reveal.
class TransformsSettingsKeys {
  final restore = GlobalKey();
  final defaults = GlobalKey();
  final appDefaults = GlobalKey();
  final favorites = GlobalKey();
  final pipelines = GlobalKey();
  final hidden = GlobalKey();
  final suggestions = GlobalKey();
  final tracking = GlobalKey();

  static const restoreId = 'result-transforms-restore';
  static const defaultsId = 'result-transforms-defaults';
  static const appDefaultsId = 'result-transforms-app-defaults';
  static const favoritesId = 'result-transforms-favorites';
  static const pipelinesId = 'result-transforms-pipelines';
  static const hiddenId = 'result-transforms-hidden';
  static const suggestionsId = 'result-transforms-suggestions';
  static const trackingId = 'result-transforms-tracking';

  List<SettingsSearchTarget> targets(BuildContext context) {
    final l10n = context.l10n;
    SettingsSearchTarget target(
      String id,
      GlobalKey key,
      String label,
      String description,
    ) => SettingsSearchTarget(
      id: id,
      section: SettingsSectionId.transforms,
      label: label,
      description: description,
      targetKey: key,
    );
    return [
      target(
        restoreId,
        restore,
        l10n.transformsRestoreTitle,
        l10n.transformsRestoreDescription,
      ),
      target(
        defaultsId,
        defaults,
        l10n.transformsDefaultTitle,
        l10n.transformsDefaultDescription,
      ),
      target(
        appDefaultsId,
        appDefaults,
        l10n.transformsAppDefaultsTitle,
        l10n.transformsAppDefaultsDescription,
      ),
      target(
        favoritesId,
        favorites,
        l10n.transformsFavoritesTitle,
        l10n.transformsFavoritesDescription,
      ),
      target(
        pipelinesId,
        pipelines,
        l10n.transformsPipelinesTitle,
        l10n.transformsPipelinesDescription,
      ),
      target(
        hiddenId,
        hidden,
        l10n.transformsHiddenTitle,
        l10n.transformsHiddenDescription,
      ),
      target(
        suggestionsId,
        suggestions,
        l10n.transformsSuggestionsTitle,
        l10n.transformsSuggestionsDescription,
      ),
      target(
        trackingId,
        tracking,
        l10n.transformsTrackingTitle,
        l10n.transformsTrackingDescription,
      ),
    ];
  }
}

/// Favorites, pipelines, paste defaults, tracking parameters, suggestions
/// and hidden transforms. It loads while shown and releases on close.
class TransformsSettingsView extends StatefulWidget {
  const TransformsSettingsView({
    super.key,
    required this.controller,
    required this.keys,
    this.scope = const SettingsSectionScope(highlightedTargetId: null),
    this.applications = const RuntimeApplicationCatalog(),
  });

  final TransformsSettingsController controller;
  final TransformsSettingsKeys keys;
  final SettingsSectionScope scope;

  /// Applications per-app defaults pick from and name.
  final ApplicationCatalog applications;

  @override
  State<TransformsSettingsView> createState() => _TransformsSettingsViewState();
}

class _TransformsSettingsViewState extends State<TransformsSettingsView> {
  String? _appPipeline;
  int _appFormRevision = 0;
  late final _applications = ApplicationDirectory(widget.applications);

  TransformsSettingsController get _controller => widget.controller;

  @override
  void initState() {
    super.initState();
    _controller.attach();
  }

  @override
  void dispose() {
    _controller.detach();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: _controller,
    builder: (context, _) {
      final l10n = context.l10n;
      switch (_controller.status) {
        case TransformsSettingsStatus.idle || TransformsSettingsStatus.loading:
          return const StateView.loading();
        case TransformsSettingsStatus.failed:
          return StateView.error(
            title: l10n.transformsLoadFailed,
            message: _controller.errorMessage,
            actionLabel: l10n.commonTryAgain,
            onAction: () => unawaited(_controller.retry()),
          );
        case TransformsSettingsStatus.ready:
          break;
      }
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (_controller.errorMessage case final error?) ...[
            Alert.destructive(
              leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
              title: Text(l10n.appActionFailed),
              content: Text(error),
            ),
            const Gap(AppSpacing.lg),
          ],
          SettingsGroup(
            children: [
              _restoreRow(context),
              _defaultRow(context),
              _appDefaultsRow(context),
            ],
          ),
          const Gap(AppSpacing.xxl),
          SettingsGroup(
            children: [
              _favoritesRow(context),
              _pipelinesRow(context),
              _hiddenRow(context),
            ],
          ),
          const Gap(AppSpacing.xxl),
          SettingsGroup(
            children: [_suggestionsRow(context), _trackingRow(context)],
          ),
        ],
      );
    },
  );

  bool _highlighted(String id) => widget.scope.isHighlighted(id);

  void _run(Future<bool> Function() change) => unawaited(change());

  Widget _restoreRow(BuildContext context) => SettingRow(
    key: widget.keys.restore,
    highlighted: _highlighted(TransformsSettingsKeys.restoreId),
    title: context.l10n.transformsRestoreTitle,
    description: context.l10n.transformsRestoreDescription,
    trailing: Switch(
      key: const ValueKey('transforms-restore'),
      value: _controller.preferences.restoreClipboardAfterPaste,
      onChanged: _controller.busy
          ? null
          : (value) => _run(() => _controller.setRestoreClipboard(value)),
    ),
  );

  Widget _pipelineSelect({
    required Key key,
    required String? value,
    required ValueChanged<String?> onChanged,
  }) {
    final l10n = context.l10n;
    const none = '';
    final candidates = _controller.defaultCandidates;
    return Select<String>(
      key: key,
      enabled: !_controller.busy,
      value: value ?? none,
      onChanged: (next) =>
          onChanged(next == null || next == none ? null : next),
      itemBuilder: (context, id) => Text(
        id == none
            ? l10n.transformsNoDefault
            : _controller.pipeline(id)?.name ?? l10n.transformsRemovedPipeline,
      ),
      popup: SelectPopup<String>(
        items: SelectItemList(
          children: [
            SelectItemButton<String>(
              value: none,
              child: Text(l10n.transformsNoDefault),
            ),
            for (final pipeline in candidates)
              SelectItemButton<String>(
                value: pipeline.id,
                child: Text(pipeline.name),
              ),
          ],
        ),
      ).call,
    );
  }

  Widget _defaultRow(BuildContext context) => SettingRow(
    key: widget.keys.defaults,
    highlighted: _highlighted(TransformsSettingsKeys.defaultsId),
    title: context.l10n.transformsDefaultTitle,
    description: context.l10n.transformsDefaultDescription,
    trailing: _pipelineSelect(
      key: const ValueKey('transforms-default'),
      value: _controller.preferences.defaultPastePipelineId,
      onChanged: (id) => _run(() => _controller.setDefaultPipeline(id)),
    ),
  );

  Widget _appDefaultsRow(BuildContext context) {
    final l10n = context.l10n;
    final entries = _controller.preferences.appDefaults;
    return SettingRow(
      key: widget.keys.appDefaults,
      highlighted: _highlighted(TransformsSettingsKeys.appDefaultsId),
      title: l10n.transformsAppDefaultsTitle,
      description: l10n.transformsAppDefaultsDescription,
      footer: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final entry in entries)
            ApplicationIdentity(
              key: ValueKey(
                'transforms-app-${entry.platform}-${entry.identifier}',
              ),
              id: entry.identifier,
              directory: _applications,
              builder: (context, app, label) => _listRow(
                leading: label,
                title: app.name,
                subtitle: [
                  if (entry.pipelineId case final id?)
                    _controller.pipeline(id)?.name ??
                        l10n.transformsRemovedPipeline
                  else
                    l10n.transformsNoDefault,
                  if (entry.platform != _controller.platform)
                    l10n.transformsOtherPlatform(entry.platform),
                ].join(' · '),
                actions: [
                  _iconButton(
                    l10n.transformsRemove,
                    LucideIcons.x,
                    _controller.busy
                        ? null
                        : () => _run(() => _controller.clearAppDefault(entry)),
                  ),
                ],
              ),
            ),
          if (entries.isNotEmpty) const Gap(AppSpacing.sm),
          Row(
            key: ValueKey('transforms-app-form-$_appFormRevision'),
            children: [
              Expanded(
                child: _pipelineSelect(
                  key: const ValueKey('transforms-app-pipeline'),
                  value: _appPipeline,
                  onChanged: (id) => setState(() => _appPipeline = id),
                ),
              ),
              const Gap(AppSpacing.sm),
              Button.secondary(
                key: const ValueKey('transforms-app-add'),
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: _controller.busy ? null : _addAppDefault,
                leading: const AppIconFrame(child: Icon(LucideIcons.plus)),
                child: Text(l10n.transformsAddApp),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _addAppDefault() async {
    final app = await showApplicationPicker(
      context,
      catalog: widget.applications,
      exclude: [
        for (final entry in _controller.preferences.appDefaults)
          if (entry.platform == _controller.platform) entry.identifier,
      ],
    );
    if (app == null || !mounted) return;
    _applications.remember(app);
    if (await _controller.setAppDefault(app.id, _appPipeline) && mounted) {
      setState(() {
        _appPipeline = null;
        _appFormRevision++;
      });
    }
  }

  Widget _favoritesRow(BuildContext context) {
    final l10n = context.l10n;
    final favorites = _controller.preferences.favorites;
    final available = [
      for (final definition in _controller.catalogue)
        TransformFavorite(TransformFavoriteKind.transform, definition.id),
      for (final pipeline in _controller.pipelines)
        TransformFavorite(TransformFavoriteKind.pipeline, pipeline.id),
    ].where((favorite) => !favorites.contains(favorite)).toList();
    return SettingRow(
      key: widget.keys.favorites,
      highlighted: _highlighted(TransformsSettingsKeys.favoritesId),
      title: l10n.transformsFavoritesTitle,
      description: l10n.transformsFavoritesDescription,
      footer: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final (index, favorite) in favorites.indexed)
            _listRow(
              title:
                  _controller.favoriteName(favorite) ??
                  l10n.transformsRemovedPipeline,
              subtitle: index < 9
                  ? l10n.transformsFavoriteShortcut(index + 1)
                  : null,
              actions: [
                _iconButton(
                  l10n.transformsMoveEarlier,
                  LucideIcons.arrowUp,
                  index > 0 && !_controller.busy
                      ? () => _run(() => _controller.moveFavorite(index, -1))
                      : null,
                ),
                _iconButton(
                  l10n.transformsMoveLater,
                  LucideIcons.arrowDown,
                  index < favorites.length - 1 && !_controller.busy
                      ? () => _run(() => _controller.moveFavorite(index, 1))
                      : null,
                ),
                _iconButton(
                  l10n.transformsRemove,
                  LucideIcons.x,
                  _controller.busy
                      ? null
                      : () => _run(() => _controller.removeFavorite(favorite)),
                ),
              ],
            ),
          if (favorites.isNotEmpty) const Gap(AppSpacing.sm),
          Select<TransformFavorite>(
            key: ValueKey('transforms-add-favorite-${favorites.length}'),
            enabled: !_controller.busy && available.isNotEmpty,
            placeholder: Text(l10n.transformsAddFavorite),
            onChanged: (favorite) {
              if (favorite != null) {
                _run(() => _controller.addFavorite(favorite));
              }
            },
            itemBuilder: (context, favorite) =>
                Text(_controller.favoriteName(favorite) ?? favorite.id),
            popup: SelectPopup<TransformFavorite>(
              items: SelectItemList(
                children: [
                  for (final favorite in available)
                    SelectItemButton<TransformFavorite>(
                      value: favorite,
                      child: Text(_controller.favoriteName(favorite) ?? ''),
                    ),
                ],
              ),
            ).call,
          ),
        ],
      ),
    );
  }

  Widget _pipelinesRow(BuildContext context) {
    final l10n = context.l10n;
    final pipelines = _controller.pipelines;
    return SettingRow(
      key: widget.keys.pipelines,
      highlighted: _highlighted(TransformsSettingsKeys.pipelinesId),
      title: l10n.transformsPipelinesTitle,
      description: l10n.transformsPipelinesDescription,
      trailing: Button.secondary(
        key: const ValueKey('transforms-new-pipeline'),
        style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
        onPressed: _controller.busy
            ? null
            : () => unawaited(
                showPipelineEditorDrawer(context, controller: _controller),
              ),
        leading: const AppIconFrame(child: Icon(LucideIcons.plus)),
        child: Text(l10n.transformsNewPipeline),
      ),
      footer: pipelines.isEmpty
          ? null
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                for (final pipeline in pipelines)
                  _listRow(
                    title: pipeline.name,
                    subtitle: _controller.pipelineUnavailable(pipeline)
                        ? l10n.transformsNeedsNewerVersion
                        : l10n.transformsPipelineSteps(pipeline.steps.length),
                    actions: [
                      _iconButton(
                        l10n.transformsEditPipeline,
                        LucideIcons.pencil,
                        _controller.busy
                            ? null
                            : () => unawaited(
                                showPipelineEditorDrawer(
                                  context,
                                  controller: _controller,
                                  pipeline: pipeline,
                                ),
                              ),
                      ),
                      _iconButton(
                        l10n.transformsDuplicatePipeline,
                        LucideIcons.copyPlus,
                        _controller.busy
                            ? null
                            : () => _run(
                                () => _controller.duplicatePipeline(pipeline),
                              ),
                      ),
                      _iconButton(
                        l10n.transformsDeletePipeline,
                        LucideIcons.trash2,
                        _controller.busy
                            ? null
                            : () => unawaited(_confirmDelete(pipeline)),
                      ),
                    ],
                  ),
              ],
            ),
    );
  }

  Future<void> _confirmDelete(TransformPipeline pipeline) =>
      AppOverlays.showActionDialog(
        context,
        icon: LucideIcons.trash2,
        title: Text(context.l10n.transformsDeletePipelineTitle(pipeline.name)),
        content: Text(context.l10n.transformsDeletePipelineMessage),
        actionLabel: context.l10n.transformsDeletePipeline,
        failureMessage: () => context.l10n.transformsDeletePipelineFailed,
        onConfirm: () async {
          if (!await _controller.deletePipeline(pipeline.id)) {
            throw StateError('pipeline not deleted');
          }
          return true;
        },
      );

  Widget _hiddenRow(BuildContext context) {
    final l10n = context.l10n;
    final hidden = _controller.preferences.hidden;
    final visible = [
      for (final definition in _controller.catalogue)
        if (!hidden.contains(definition.id)) definition,
    ];
    return SettingRow(
      key: widget.keys.hidden,
      highlighted: _highlighted(TransformsSettingsKeys.hiddenId),
      title: l10n.transformsHiddenTitle,
      description: l10n.transformsHiddenDescription,
      footer: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final id in hidden)
            _listRow(
              title: _controller.definition(id)?.name ?? id,
              actions: [
                _iconButton(
                  l10n.transformsShow,
                  LucideIcons.eye,
                  _controller.busy
                      ? null
                      : () => _run(() => _controller.setHidden(id, false)),
                ),
              ],
            ),
          if (hidden.isNotEmpty) const Gap(AppSpacing.sm),
          Select<String>(
            key: ValueKey('transforms-hide-${hidden.length}'),
            enabled: !_controller.busy && visible.isNotEmpty,
            placeholder: Text(l10n.transformsHideTransform),
            onChanged: (id) {
              if (id != null) _run(() => _controller.setHidden(id, true));
            },
            itemBuilder: (context, id) =>
                Text(_controller.definition(id)?.name ?? id),
            popup: SelectPopup<String>(
              items: SelectItemList(
                children: [
                  for (final definition in visible)
                    SelectItemButton<String>(
                      value: definition.id,
                      child: Text(definition.name),
                    ),
                ],
              ),
            ).call,
          ),
        ],
      ),
    );
  }

  Widget _suggestionsRow(BuildContext context) {
    final l10n = context.l10n;
    final preferences = _controller.preferences;
    return SettingRow(
      key: widget.keys.suggestions,
      highlighted: _highlighted(TransformsSettingsKeys.suggestionsId),
      title: l10n.transformsSuggestionsTitle,
      description: l10n.transformsSuggestionsDescription,
      trailing: Switch(
        key: const ValueKey('transforms-suggestions'),
        value: preferences.suggestionsEnabled,
        onChanged: _controller.busy
            ? null
            : (value) => _run(() => _controller.setSuggestions(value)),
      ),
      footer: !preferences.suggestionsEnabled
          ? null
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                for (final detector in TransformDetector.values)
                  Padding(
                    padding: const EdgeInsets.symmetric(
                      vertical: AppSpacing.xs,
                    ),
                    child: MergeSemantics(
                      child: Row(
                        children: [
                          Expanded(
                            child: Text(_detectorLabel(detector)).small(),
                          ),
                          AppSwitchFrame(
                            child: Switch(
                              value: !preferences.disabledDetectors.contains(
                                detector.wireName,
                              ),
                              onChanged: _controller.busy
                                  ? null
                                  : (value) => _run(
                                      () => _controller.setDetector(
                                        detector,
                                        value,
                                      ),
                                    ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
              ],
            ),
    );
  }

  String _detectorLabel(TransformDetector detector) {
    final l10n = context.l10n;
    return switch (detector) {
      TransformDetector.json => l10n.transformsDetectorJson,
      TransformDetector.jwt => l10n.transformsDetectorJwt,
      TransformDetector.base64 => l10n.transformsDetectorBase64,
      TransformDetector.trackingUrl => l10n.transformsDetectorTrackingUrl,
      TransformDetector.timestamp => l10n.transformsDetectorTimestamp,
      TransformDetector.html => l10n.transformsDetectorHtml,
      TransformDetector.markdown => l10n.transformsDetectorMarkdown,
    };
  }

  Widget _trackingRow(BuildContext context) {
    final l10n = context.l10n;
    final preferences = _controller.preferences;
    return SettingRow(
      key: widget.keys.tracking,
      highlighted: _highlighted(TransformsSettingsKeys.trackingId),
      title: l10n.transformsTrackingTitle,
      description: l10n.transformsTrackingDescription,
      footer: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _TrackingList(
            key: const ValueKey('transforms-tracking-remove'),
            label: l10n.transformsTrackingRemoveLabel,
            members: preferences.trackingRemove,
            enabled: !_controller.busy,
            onAdd: (member) => _controller.setTrackingParameter(
              TransformSet.trackingRemove,
              member,
              present: true,
            ),
            onRemove: (member) => _run(
              () => _controller.setTrackingParameter(
                TransformSet.trackingRemove,
                member,
                present: false,
              ),
            ),
            removeLabel: l10n.transformsRemove,
          ),
          const Gap(AppSpacing.md),
          _TrackingList(
            key: const ValueKey('transforms-tracking-allow'),
            label: l10n.transformsTrackingAllowLabel,
            members: preferences.trackingAllow,
            enabled: !_controller.busy,
            onAdd: (member) => _controller.setTrackingParameter(
              TransformSet.trackingAllow,
              member,
              present: true,
            ),
            onRemove: (member) => _run(
              () => _controller.setTrackingParameter(
                TransformSet.trackingAllow,
                member,
                present: false,
              ),
            ),
            removeLabel: l10n.transformsRemove,
          ),
        ],
      ),
    );
  }

  /// A [leading] widget replaces the [title] text.
  Widget _listRow({
    Widget? leading,
    required String title,
    String? subtitle,
    required List<Widget> actions,
  }) => Padding(
    padding: const EdgeInsets.symmetric(vertical: AppSpacing.xxs),
    child: Row(
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              (leading ??
                      Text(title, maxLines: 1, overflow: TextOverflow.ellipsis))
                  .small(),
              if (subtitle != null)
                Text(
                  subtitle,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ).xSmall().muted(),
            ],
          ),
        ),
        ...actions,
      ],
    ),
  );

  Widget _iconButton(String label, IconData icon, VoidCallback? onPressed) =>
      Tooltip(
        tooltip: (context) => TooltipContainer(child: Text(label)),
        child: Button.ghost(
          style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
          onPressed: onPressed,
          child: Semantics(
            label: label,
            child: AppIconFrame(child: Icon(icon)),
          ),
        ),
      );
}

/// One editable list of tracking parameters, `name` or `name@domain`.
class _TrackingList extends StatefulWidget {
  const _TrackingList({
    super.key,
    required this.label,
    required this.members,
    required this.enabled,
    required this.onAdd,
    required this.onRemove,
    required this.removeLabel,
  });

  final String label;
  final List<String> members;
  final bool enabled;
  final Future<bool> Function(String member) onAdd;
  final ValueChanged<String> onRemove;
  final String removeLabel;

  @override
  State<_TrackingList> createState() => _TrackingListState();
}

class _TrackingListState extends State<_TrackingList> {
  final _field = TextEditingController();

  @override
  void dispose() {
    _field.dispose();
    super.dispose();
  }

  Future<void> _add() async {
    final member = _field.text.trim();
    if (member.isEmpty || !widget.enabled) return;
    if (await widget.onAdd(member) && mounted) _field.clear();
  }

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Text(widget.label).small(),
      const Gap(AppSpacing.xs),
      if (widget.members.isNotEmpty) ...[
        Wrap(
          spacing: AppSpacing.xs,
          runSpacing: AppSpacing.xs,
          children: [
            for (final member in widget.members)
              Chip(
                trailing: ChipButton(
                  onPressed: widget.enabled
                      ? () => widget.onRemove(member)
                      : null,
                  child: Semantics(
                    label: widget.removeLabel,
                    child: const Icon(LucideIcons.x, size: AppIconSize.xs),
                  ),
                ),
                child: Text(member),
              ),
          ],
        ),
        const Gap(AppSpacing.xs),
      ],
      TextField(
        controller: _field,
        enabled: widget.enabled,
        autocorrect: false,
        enableSuggestions: false,
        placeholder: Text(context.l10n.transformsTrackingPlaceholder),
        decoration: AppOverlays.dialogFieldDecoration(context),
        onSubmitted: (_) => unawaited(_add()),
      ),
    ],
  );
}
