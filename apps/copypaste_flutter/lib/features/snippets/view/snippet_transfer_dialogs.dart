import 'dart:async';
import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../platform/files/platform_file_gateway.dart';
import '../controller/snippets_controller.dart';
import '../models/snippet_models.dart';

/// Extension of an exported snippet bundle.
const snippetBundleExtension = 'copypaste-snippets';

/// Native file choice for snippet transfer; Android stages files in the app
/// cache and shares created ones.
class SnippetTransferFiles {
  const SnippetTransferFiles({this.gateway = const PlatformFileGateway()});

  final PlatformFileGateway gateway;

  Future<String?> chooseImport(AppLocalizations l10n) async {
    final selected = await openFile(
      acceptedTypeGroups: [
        XTypeGroup(
          label: l10n.snippetsImportFileType,
          extensions: const [snippetBundleExtension, 'json', 'csv'],
        ),
      ],
    );
    if (selected == null) return null;
    if (!Platform.isAndroid) return selected.path;
    final staged = await gateway.androidStagingPath(selected.name);
    await selected.saveTo(staged);
    return staged;
  }

  Future<String?> chooseExport() async {
    const name = 'copypaste-snippets.$snippetBundleExtension';
    if (Platform.isAndroid) return gateway.androidStagingPath(name);
    return (await getSaveLocation(suggestedName: name))?.path;
  }

  Future<void> presentExport(String path) =>
      gateway.presentCreatedFile(path, mimeType: 'application/json');
}

SnippetImportFormat snippetImportFormatFor(String path) {
  final lower = path.toLowerCase();
  if (lower.endsWith('.csv')) return SnippetImportFormat.csv;
  if (lower.endsWith('.json')) return SnippetImportFormat.json;
  return SnippetImportFormat.bundle;
}

/// Preview, then import: counts and invalid rows first, with the conflict
/// policies, then an atomic commit. Closing without importing releases the
/// plan.
Future<void> showSnippetImportDialog(
  BuildContext context, {
  required SnippetsController controller,
  required String path,
}) => AppOverlays.showDialog<void>(
  context,
  barrierDismissible: false,
  builder: (_) => _ImportDialog(controller: controller, path: path),
);

class _ImportDialog extends StatefulWidget {
  const _ImportDialog({required this.controller, required this.path});

  final SnippetsController controller;
  final String path;

  @override
  State<_ImportDialog> createState() => _ImportDialogState();
}

class _ImportDialogState extends State<_ImportDialog> {
  late SnippetImportFormat _format = snippetImportFormatFor(widget.path);
  String _passphrase = '';
  bool _convert = true;
  SnippetImportPlan? _plan;
  SnippetImportPolicies _policies = const SnippetImportPolicies();
  bool _working = false;
  String? _error;

  @override
  void dispose() {
    final token = _plan?.token;
    if (token != null) {
      unawaited(
        widget.controller.repository
            .cancelImport(token)
            .catchError((Object _) {}),
      );
    }
    _passphrase = '';
    super.dispose();
  }

  Future<void> _run(Future<void> Function() work) async {
    setState(() {
      _working = true;
      _error = null;
    });
    try {
      await work();
    } on SnippetException catch (error) {
      _error = error.message;
    } on Object {
      _error = AppStrings.current.snippetsImportFailed;
    } finally {
      if (mounted) setState(() => _working = false);
    }
  }

  Future<void> _preview() => _run(() async {
    final plan = await widget.controller.repository.previewImport(
      widget.path,
      format: _format,
      passphrase: _format == SnippetImportFormat.bundle && _passphrase.isNotEmpty
          ? _passphrase
          : null,
      convertPlaceholders: _format != SnippetImportFormat.bundle && _convert,
    );
    if (mounted) setState(() => _plan = plan);
  });

  Future<void> _commit() => _run(() async {
    final plan = _plan!;
    final report = await widget.controller.repository.commitImport(
      plan.token,
      _policies,
    );
    if (!mounted) return;
    _plan = null;
    final l10n = context.l10n;
    AppToast.show(
      context,
      title: l10n.snippetsImportDone,
      message: l10n.snippetsImportReport(
        report.inserted,
        report.updated,
        report.skipped,
        report.withoutAbbreviation + report.renamedAbbreviations,
        report.unrecognizedPlaceholders,
      ),
      tone: AppToastTone.success,
    );
    Navigator.pop(context);
  });

  Widget _label(String text) => Padding(
    padding: const EdgeInsets.only(bottom: AppSpacing.xs),
    child: Text(text).small(),
  );

  Widget _select<T>(
    String label,
    T value,
    List<T> values,
    String Function(T) name,
    ValueChanged<T> onChanged,
  ) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      _label(label),
      Select<T>(
        value: value,
        enabled: !_working,
        onChanged: (next) {
          if (next != null) setState(() => onChanged(next));
        },
        itemBuilder: (context, item) => Text(name(item)),
        popup: SelectPopup<T>(
          items: SelectItemList(
            children: [
              for (final item in values)
                SelectItemButton<T>(value: item, child: Text(name(item))),
            ],
          ),
        ).call,
      ),
      const Gap(AppSpacing.md),
    ],
  );

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final plan = _plan;
    return AppOverlays.alertDialog(
      icon: LucideIcons.fileInput,
      title: Text(l10n.snippetsImportTitle),
      content: ConstrainedBox(
        constraints: BoxConstraints(
          maxHeight:
              MediaQuery.sizeOf(context).height *
              AppOverlaySize.dialogContentHeightFactor,
        ),
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (_error case final error?) ...[
                Alert.destructive(
                  leading: const AppIconFrame(
                    child: Icon(LucideIcons.circleAlert),
                  ),
                  title: Text(l10n.snippetsImportFailed),
                  content: Text(error),
                ),
                const Gap(AppSpacing.md),
              ],
              if (plan == null) ..._options(l10n) else ..._summary(l10n, plan),
            ],
          ),
        ),
      ),
      actions: [
        Button.ghost(
          alignment: Alignment.center,
          style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
          onPressed: _working ? null : () => Navigator.pop(context),
          child: Text(l10n.commonCancel),
        ),
        Button.primary(
          key: const ValueKey('snippet-import-confirm'),
          alignment: Alignment.center,
          style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
          onPressed: _working
              ? null
              : plan == null
              ? () => unawaited(_preview())
              : plan.counts.importable == 0
              ? null
              : () => unawaited(_commit()),
          child: Text(
            plan == null ? l10n.snippetsImportPreview : l10n.snippetsImport,
          ),
        ),
      ],
    );
  }

  List<Widget> _options(AppLocalizations l10n) => [
    Text(widget.path.split(Platform.pathSeparator).last).muted(),
    const Gap(AppSpacing.md),
    _select(
      l10n.snippetsImportFormat,
      _format,
      SnippetImportFormat.values,
      (format) => l10n.snippetsImportFormatName(format.name),
      (format) => _format = format,
    ),
    if (_format == SnippetImportFormat.bundle) ...[
      _label(l10n.snippetsPassphraseLabel),
      TextField(
        key: const ValueKey('snippet-import-passphrase'),
        obscureText: true,
        autocorrect: false,
        enableSuggestions: false,
        placeholder: Text(l10n.snippetsImportPassphraseHint),
        decoration: AppOverlays.dialogFieldDecoration(context),
        onChanged: (value) => _passphrase = value,
      ),
    ] else
      MergeSemantics(
        child: Row(
          children: [
            Expanded(child: Text(l10n.snippetsImportConvert)),
            AppSwitchFrame(
              child: Switch(
                value: _convert,
                onChanged: (value) => setState(() => _convert = value),
              ),
            ),
          ],
        ),
      ),
  ];

  List<Widget> _summary(AppLocalizations l10n, SnippetImportPlan plan) {
    final counts = plan.counts;
    final invalid = [
      for (final row in plan.rows)
        if (row.outcome == SnippetImportRowOutcome.invalid) row,
    ];
    return [
      Text(
        l10n.snippetsImportCounts(
          counts.added,
          counts.identical,
          counts.updated,
          counts.abbreviationConflicts,
          counts.invalid,
        ),
        key: const ValueKey('snippet-import-counts'),
      ),
      const Gap(AppSpacing.md),
      if (counts.updated > 0)
        _select(
          l10n.snippetsImportSameIdPolicy,
          _policies.sameIdDifferent,
          SnippetSameIdPolicy.values,
          (policy) => l10n.snippetsImportSameIdPolicyName(policy.name),
          (policy) => _policies = _policies.copyWith(sameIdDifferent: policy),
        ),
      if (counts.abbreviationConflicts > 0)
        _select(
          l10n.snippetsImportAbbreviationPolicy,
          _policies.abbreviationConflict,
          SnippetAbbreviationPolicy.values,
          (policy) => l10n.snippetsImportAbbreviationPolicyName(policy.name),
          (policy) =>
              _policies = _policies.copyWith(abbreviationConflict: policy),
        ),
      if (counts.collections > 0)
        _select(
          l10n.snippetsImportCollectionPolicy,
          _policies.collectionName,
          SnippetCollectionNamePolicy.values,
          (policy) => l10n.snippetsImportCollectionPolicyName(policy.name),
          (policy) => _policies = _policies.copyWith(collectionName: policy),
        ),
      if (invalid.isNotEmpty) ...[
        Text(l10n.snippetsImportInvalidRows).small(),
        const Gap(AppSpacing.xs),
        for (final row in invalid.take(50))
          Text(
            l10n.snippetsImportInvalidRow(row.index + 1, row.reason ?? ''),
          ).small().muted(),
      ],
    ];
  }
}

/// Exports all snippets, the [collectionId] collection or [snippetIds].
/// Encryption is on by default; a plain file and concealed snippets in it
/// each need their own confirmation.
Future<void> showSnippetExportDialog(
  BuildContext context, {
  required SnippetsController controller,
  SnippetTransferFiles files = const SnippetTransferFiles(),
  String? collectionId,
  List<String> snippetIds = const [],
}) => AppOverlays.showDialog<void>(
  context,
  builder: (_) => _ExportDialog(
    controller: controller,
    files: files,
    collectionId: collectionId,
    snippetIds: snippetIds,
  ),
);

class _ExportDialog extends StatefulWidget {
  const _ExportDialog({
    required this.controller,
    required this.files,
    this.collectionId,
    this.snippetIds = const [],
  });

  final SnippetsController controller;
  final SnippetTransferFiles files;
  final String? collectionId;
  final List<String> snippetIds;

  @override
  State<_ExportDialog> createState() => _ExportDialogState();
}

class _ExportDialogState extends State<_ExportDialog> {
  late SnippetExportScopeKind _scope = widget.snippetIds.isNotEmpty
      ? SnippetExportScopeKind.snippets
      : widget.collectionId != null
      ? SnippetExportScopeKind.collections
      : SnippetExportScopeKind.all;
  bool _encrypt = true;
  String _passphrase = '';
  String _confirm = '';
  bool _plainConfirmed = false;
  bool _concealedConfirmed = false;
  bool _working = false;
  String? _error;

  /// 0 to 4 from length and character classes.
  static int strength(String passphrase) {
    if (passphrase.length < SnippetLimits.minPassphrase) return 0;
    var score = 1;
    if (passphrase.length >= 12) score++;
    if (passphrase.length >= 16) score++;
    final classes = [
      RegExp('[a-z]'),
      RegExp('[A-Z]'),
      RegExp('[0-9]'),
      RegExp('[^a-zA-Z0-9]'),
    ].where((pattern) => pattern.hasMatch(passphrase)).length;
    if (classes >= 3) score++;
    return score.clamp(0, 4);
  }

  bool get _valid => _encrypt
      ? _passphrase.length >= SnippetLimits.minPassphrase &&
            _passphrase == _confirm
      : _plainConfirmed;

  @override
  void dispose() {
    _passphrase = '';
    _confirm = '';
    super.dispose();
  }

  Future<void> _export() async {
    final l10n = context.l10n;
    setState(() {
      _working = true;
      _error = null;
    });
    try {
      final path = await widget.files.chooseExport();
      if (path == null) return;
      final count = await widget.controller.repository.export(
        SnippetExportRequest(
          path: path,
          scope: _scope,
          ids: switch (_scope) {
            SnippetExportScopeKind.all => const [],
            SnippetExportScopeKind.collections => [?widget.collectionId],
            SnippetExportScopeKind.snippets => widget.snippetIds,
          },
          passphrase: _encrypt ? _passphrase : null,
          includeConcealed: _encrypt || _concealedConfirmed,
        ),
      );
      await widget.files.presentExport(path);
      if (!mounted) return;
      AppToast.show(
        context,
        title: l10n.snippetsExportDone,
        message: l10n.snippetsExportCount(count),
        tone: AppToastTone.success,
      );
      Navigator.pop(context);
    } on SnippetException catch (error) {
      _error = error.message;
    } on Object {
      _error = l10n.snippetsExportFailed;
    } finally {
      if (mounted) setState(() => _working = false);
    }
  }

  Widget _switch(String label, bool value, ValueChanged<bool> onChanged) =>
      MergeSemantics(
        child: Row(
          children: [
            Expanded(child: Text(label)),
            const Gap(AppSpacing.md),
            AppSwitchFrame(
              child: Switch(
                value: value,
                onChanged: _working
                    ? null
                    : (next) => setState(() => onChanged(next)),
              ),
            ),
          ],
        ),
      );

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final scopes = [
      SnippetExportScopeKind.all,
      if (widget.collectionId != null) SnippetExportScopeKind.collections,
      if (widget.snippetIds.isNotEmpty) SnippetExportScopeKind.snippets,
    ];
    final score = strength(_passphrase);
    return AppOverlays.alertDialog(
      icon: LucideIcons.fileOutput,
      title: Text(l10n.snippetsExportTitle),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (_error case final error?) ...[
            Alert.destructive(
              leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
              title: Text(l10n.snippetsExportFailed),
              content: Text(error),
            ),
            const Gap(AppSpacing.md),
          ],
          if (scopes.length > 1) ...[
            Select<SnippetExportScopeKind>(
              value: _scope,
              onChanged: (value) {
                if (value != null) setState(() => _scope = value);
              },
              itemBuilder: (context, scope) => Text(
                l10n.snippetsExportScope(
                  scope.name,
                  widget.snippetIds.length,
                ),
              ),
              popup: SelectPopup<SnippetExportScopeKind>(
                items: SelectItemList(
                  children: [
                    for (final scope in scopes)
                      SelectItemButton<SnippetExportScopeKind>(
                        value: scope,
                        child: Text(
                          l10n.snippetsExportScope(
                            scope.name,
                            widget.snippetIds.length,
                          ),
                        ),
                      ),
                  ],
                ),
              ).call,
            ),
            const Gap(AppSpacing.md),
          ],
          _switch(l10n.snippetsExportEncrypt, _encrypt, (value) {
            _encrypt = value;
          }),
          const Gap(AppSpacing.md),
          if (_encrypt) ...[
            TextField(
              key: const ValueKey('snippet-export-passphrase'),
              obscureText: true,
              autocorrect: false,
              enableSuggestions: false,
              placeholder: Text(
                l10n.snippetsPassphraseHint(SnippetLimits.minPassphrase),
              ),
              decoration: AppOverlays.dialogFieldDecoration(context),
              onChanged: (value) => setState(() => _passphrase = value),
            ),
            const Gap(AppSpacing.sm),
            LinearProgressIndicator(value: score / 4),
            const Gap(AppSpacing.xs),
            Text(l10n.snippetsPassphraseStrength(score)).small().muted(),
            const Gap(AppSpacing.sm),
            TextField(
              key: const ValueKey('snippet-export-confirm'),
              obscureText: true,
              autocorrect: false,
              enableSuggestions: false,
              placeholder: Text(l10n.snippetsPassphraseConfirm),
              decoration: AppOverlays.dialogFieldDecoration(context),
              onChanged: (value) => setState(() => _confirm = value),
            ),
            const Gap(AppSpacing.xs),
            Text(l10n.snippetsPassphraseNoRecovery).small().muted(),
          ] else ...[
            _switch(l10n.snippetsExportPlainConfirm, _plainConfirmed, (value) {
              _plainConfirmed = value;
            }),
            const Gap(AppSpacing.sm),
            _switch(
              l10n.snippetsExportConcealedConfirm,
              _concealedConfirmed,
              (value) => _concealedConfirmed = value,
            ),
          ],
        ],
      ),
      actions: [
        Button.ghost(
          alignment: Alignment.center,
          style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
          onPressed: _working ? null : () => Navigator.pop(context),
          child: Text(l10n.commonCancel),
        ),
        Button.primary(
          key: const ValueKey('snippet-export-confirm-action'),
          alignment: Alignment.center,
          style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
          onPressed: _working || !_valid ? null : () => unawaited(_export()),
          child: Text(l10n.snippetsExport),
        ),
      ],
    );
  }
}
