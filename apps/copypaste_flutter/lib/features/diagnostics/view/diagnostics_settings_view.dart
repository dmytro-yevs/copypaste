import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../../shared/system_date_time.dart';
import '../../settings/controller/settings_navigation_state.dart';
import '../../settings/view/settings_search_target.dart';
import '../../settings/view/settings_section.dart';
import '../controller/diagnostics_controller.dart';
import '../models/diagnostic_models.dart';

/// Settings → Diagnostics as a settings screen contribution.
SettingsFeatureSection diagnosticsSettingsSection(
  DiagnosticsController controller,
) => SettingsFeatureSection(
  section: SettingsSectionId.diagnostics,
  builder: (context, scope) => DiagnosticsSettingsView(controller: controller),
);

/// The redacted runtime log, newest first, with filters and export. It loads
/// while shown and releases its rows on close.
class DiagnosticsSettingsView extends StatefulWidget {
  const DiagnosticsSettingsView({super.key, required this.controller});

  final DiagnosticsController controller;

  @override
  State<DiagnosticsSettingsView> createState() =>
      _DiagnosticsSettingsViewState();
}

class _DiagnosticsSettingsViewState extends State<DiagnosticsSettingsView> {
  DiagnosticsController get _controller => widget.controller;

  /// Android runs no daemon, so there is nothing to tell apart.
  bool get _includesDaemon => defaultTargetPlatform != TargetPlatform.android;

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

  Future<void> _export() async {
    final l10n = context.l10n;
    final outcome = await _controller.export();
    if (!mounted || outcome == DiagnosticExportOutcome.cancelled) return;
    final saved = outcome == DiagnosticExportOutcome.saved;
    AppToast.show(
      context,
      title: saved ? l10n.diagnosticsExported : l10n.diagnosticsExportFailed,
      message: saved
          ? l10n.diagnosticsExportedMessage
          : l10n.diagnosticsExportFailedMessage,
      tone: saved ? AppToastTone.success : AppToastTone.error,
    );
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: _controller,
      builder: (context, _) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [_toolbar(context), const Gap(AppSpacing.lg), _body(context)],
      ),
    );
  }

  Widget _toolbar(BuildContext context) {
    final l10n = context.l10n;
    final filter = _controller.filter;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        TextField(
          key: const ValueKey('diagnostics-search'),
          decoration: AppTheme.searchFieldDecoration(context),
          initialValue: filter.query,
          placeholder: Text(l10n.diagnosticsSearchPlaceholder),
          onChanged: _controller.setQuery,
          skipInputFeatureFocusTraversal: true,
          features: const [
            InputFeature.leading(AppIconFrame(child: Icon(LucideIcons.search))),
            InputFeature.clear(),
          ],
        ),
        const Gap(AppSpacing.sm),
        Wrap(
          spacing: AppSpacing.sm,
          runSpacing: AppSpacing.sm,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            _select<DiagnosticSeverity>(
              context,
              key: const ValueKey('diagnostics-severity'),
              allLabel: l10n.diagnosticsAllLevels,
              values: DiagnosticSeverity.values,
              value: filter.severity,
              label: (severity) => _severityLabel(context, severity),
              allIcon: LucideIcons.listFilter,
              icon: (severity) => switch (severity) {
                DiagnosticSeverity.error => LucideIcons.circleX,
                DiagnosticSeverity.warn => LucideIcons.triangleAlert,
                DiagnosticSeverity.info => LucideIcons.info,
                DiagnosticSeverity.debug => LucideIcons.bug,
                DiagnosticSeverity.trace => LucideIcons.footprints,
              },
              onChanged: _controller.setSeverity,
            ),
            if (_includesDaemon)
              _select<DiagnosticSource>(
                context,
                key: const ValueKey('diagnostics-source'),
                allLabel: l10n.diagnosticsAllProcesses,
                values: DiagnosticSource.values,
                value: filter.source,
                label: (source) => _sourceLabel(context, source),
                allIcon: LucideIcons.layers,
                icon: (source) => switch (source) {
                  DiagnosticSource.app => LucideIcons.appWindow,
                  DiagnosticSource.daemon => LucideIcons.server,
                },
                onChanged: _controller.setSource,
              ),
            Tooltip(
              tooltip: (context) =>
                  TooltipContainer(child: Text(l10n.diagnosticsRefresh)),
              child: Button.secondary(
                key: const ValueKey('diagnostics-refresh'),
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.secondaryIcon(),
                ),
                onPressed: () => unawaited(_controller.refresh()),
                child: Semantics(
                  label: l10n.diagnosticsRefresh,
                  child: const AppIconFrame(child: Icon(LucideIcons.refreshCw)),
                ),
              ),
            ),
            Button.secondary(
              key: const ValueKey('diagnostics-export'),
              style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
              onPressed: _controller.exporting
                  ? null
                  : () => unawaited(_export()),
              leading: const AppIconFrame(child: Icon(LucideIcons.download)),
              child: Text(l10n.diagnosticsExport),
            ),
          ],
        ),
      ],
    );
  }

  /// A filter whose empty choice lists everything.
  Widget _select<T extends Enum>(
    BuildContext context, {
    required Key key,
    required String allLabel,
    required List<T> values,
    required T? value,
    required String Function(T value) label,
    required IconData allIcon,
    required IconData Function(T value) icon,
    required ValueChanged<T?> onChanged,
  }) {
    Widget option(String id) {
      final value = id.isEmpty
          ? null
          : values.firstWhere((value) => value.name == id);
      return Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          AppIconFrame(
            size: AppIconSize.sm,
            child: Icon(
              value == null ? allIcon : icon(value),
              size: AppIconSize.sm,
            ),
          ),
          const Gap(AppSpacing.sm),
          Text(value == null ? allLabel : label(value), maxLines: 1),
        ],
      );
    }

    return ComponentTheme<SelectTheme>(
      data: AppTheme.selectTheme(
        context,
        variant: AppSelectVariant.toolbar,
        labels: [allLabel, for (final value in values) label(value)],
      ),
      child: Select<String>(
        key: key,
        filled: true,
        value: value?.name ?? '',
        onChanged: (id) {
          if (id == null) return;
          onChanged(
            id.isEmpty ? null : values.firstWhere((value) => value.name == id),
          );
        },
        itemBuilder: (context, id) => option(id),
        popup: SelectPopup<String>.noVirtualization(
          items: SelectItemList(
            children: [
              SelectItemButton<String>(value: '', child: option('')),
              for (final value in values)
                SelectItemButton<String>(
                  value: value.name,
                  child: option(value.name),
                ),
            ],
          ),
        ).call,
      ),
    );
  }

  Widget _body(BuildContext context) {
    final l10n = context.l10n;
    switch (_controller.status) {
      case DiagnosticsStatus.idle || DiagnosticsStatus.loading:
        return const StateView.loading();
      case DiagnosticsStatus.failed:
        return StateView.error(
          title: l10n.diagnosticsLoadFailed,
          onRetry: () => unawaited(_controller.refresh()),
        );
      case DiagnosticsStatus.ready:
        break;
    }
    final entries = _controller.entries;
    if (entries.isEmpty) {
      return StateView.empty(
        key: const ValueKey('diagnostics-empty'),
        title: l10n.diagnosticsEmpty,
        icon: LucideIcons.scrollText,
        compact: true,
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SettingsGroup(
          key: const ValueKey('diagnostics-list'),
          children: [for (final entry in entries) _row(context, entry)],
        ),
        if (_controller.pageCount > 1) ...[
          const Gap(AppSpacing.lg),
          Center(
            child: Pagination(
              key: const ValueKey('diagnostics-pagination'),
              // The library counts pages from one.
              page: _controller.page + 1,
              totalPages: _controller.pageCount,
              onPageChanged: (page) =>
                  unawaited(_controller.openPage(page - 1)),
              maxPages: AppTheme.paginationMaxPages,
              hidePreviousOnFirstPage: true,
              hideNextOnLastPage: true,
            ),
          ),
        ],
      ],
    );
  }

  Widget _row(BuildContext context, DiagnosticEntry entry) {
    final label = Text(_severityLabel(context, entry.severity));
    return Padding(
      padding: AppTheme.settingsRowPadding,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SecondaryBadge(
            style: AppTheme.statusBadgeStyle(context, switch (entry.severity) {
              DiagnosticSeverity.error => AppStatusTone.error,
              DiagnosticSeverity.warn => AppStatusTone.warning,
              DiagnosticSeverity.info => AppStatusTone.info,
              DiagnosticSeverity.debug ||
              DiagnosticSeverity.trace => AppStatusTone.muted,
            }),
            child: label,
          ),
          const Gap(AppSpacing.md),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(entry.message).small(),
                const Gap(AppSpacing.xs),
                Text(
                  [
                    formatSystemTimestamp(context, entry.at),
                    if (_includesDaemon) _sourceLabel(context, entry.source),
                    entry.target,
                  ].join(' · '),
                ).xSmall().muted(),
              ],
            ),
          ),
        ],
      ),
    );
  }

  String _severityLabel(BuildContext context, DiagnosticSeverity severity) {
    final l10n = context.l10n;
    return switch (severity) {
      DiagnosticSeverity.error => l10n.diagnosticsLevelError,
      DiagnosticSeverity.warn => l10n.diagnosticsLevelWarning,
      DiagnosticSeverity.info => l10n.diagnosticsLevelInfo,
      DiagnosticSeverity.debug => l10n.diagnosticsLevelDebug,
      DiagnosticSeverity.trace => l10n.diagnosticsLevelTrace,
    };
  }

  String _sourceLabel(BuildContext context, DiagnosticSource source) =>
      switch (source) {
        DiagnosticSource.app => context.l10n.diagnosticsProcessApp,
        DiagnosticSource.daemon => context.l10n.diagnosticsProcessDaemon,
      };
}
