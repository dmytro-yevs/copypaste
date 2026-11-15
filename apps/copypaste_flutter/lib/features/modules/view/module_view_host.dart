import 'dart:async';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../platform/security/owner_access.dart';
import '../../../shared/error_text.dart';
import '../../../shared/failure_toasts.dart';
import '../../../shared/security/owner_verification.dart';
import '../../../shared/forms/dynamic_form_draft.dart';
import '../../../shared/forms/dynamic_form_fields.dart';
import '../../settings/view/settings_section.dart';
import '../controller/module_view_controller.dart';
import '../controller/module_view_services.dart';
import '../controller/modules_controller.dart';
import '../models/module_models.dart';
import '../models/module_view.dart';
import 'module_command_dialogs.dart';
import 'module_picker_fields.dart';
import 'module_view_form.dart';
import 'module_view_sections.dart';

/// Shows the view of [controller] and runs its actions: confirmation,
/// owner verification, drawers, and the results a view does not replace.
///
/// Inside a drawer ([inDrawer]), drawer targets stack in place instead of
/// opening another drawer, and a result that is not a view finishes the
/// drawer through [onFinished].
class ModuleViewHost extends StatefulWidget {
  const ModuleViewHost({
    super.key,
    required this.controller,
    this.services = const ModuleViewServices(),
    this.enabled = true,
    this.inDrawer = false,
    this.onFinished,
  });

  final ModuleViewController controller;
  final ModuleViewServices services;
  final bool enabled;
  final bool inDrawer;
  final ValueChanged<ModuleResult>? onFinished;

  @override
  State<ModuleViewHost> createState() => _ModuleViewHostState();
}

class _ModuleViewHostState extends State<ModuleViewHost> {
  late final ModulePickerFields _pickers = ModulePickerFields(
    controller: widget.controller.modules,
    applications: widget.services.applications,
  );
  OwnerAccess? _access;

  /// Views a drawer target opened while this host is itself in a drawer.
  final _layers = <ModuleViewController>[];

  ModuleViewController get _top => _layers.lastOrNull ?? widget.controller;
  ModulesController get _modules => widget.controller.modules;

  @override
  void dispose() {
    _access?.lock();
    for (final layer in _layers) {
      layer.dispose();
    }
    super.dispose();
  }

  Future<bool> _ensureOwner() {
    final authenticator = widget.services.authenticator;
    return verifyDeviceOwner(
      context,
      access: _access ??= authenticator == null
          ? null
          : OwnerAccess(authenticator: authenticator),
    );
  }

  Future<void> _act(ModuleViewAction action) async {
    final views = _top;
    if (action.requireAuth && !await _ensureOwner()) return;
    if (!mounted) return;
    final drawer = action.target == ModuleViewTarget.drawer;
    ModuleResult? result;
    Future<bool> run() async {
      result = await views.run(action.command, action.arguments, show: !drawer);
      return views.errorMessage == null;
    }

    if (action.destructive) {
      final done = await AppOverlays.showActionDialog(
        context,
        icon: LucideIcons.puzzle,
        title: Text(action.title),
        actionLabel: action.title,
        changes: views,
        canConfirm: () => !views.busy,
        onConfirm: run,
        failureMessage: () =>
            views.errorMessage ?? AppStrings.current.modulesOperationIncomplete,
      );
      if (done != true) return;
    } else {
      await run();
    }
    final output = result;
    if (output == null || !mounted) return;
    if (output.view case final view?) {
      await _openDrawer(view, action);
      return;
    }
    if (_layers.isNotEmpty) {
      setState(() => _layers.removeLast().dispose());
      unawaited(_top.refresh());
    } else if (widget.onFinished case final finish?) {
      finish(output);
      return;
    }
    if (mounted) await _present(output, action.title);
  }

  Future<void> _openDrawer(ModuleView view, ModuleViewAction action) async {
    final layer = ModuleViewController(
      modules: _modules,
      moduleId: widget.controller.moduleId,
      view: view,
      source: (action.command, action.arguments),
      transient: true,
    );
    if (widget.inDrawer) {
      setState(() => _layers.add(layer));
      return;
    }
    final refresh = widget.controller.refresh;
    await showModuleViewDrawer(
      context,
      title: action.title,
      controller: layer,
      services: widget.services,
    );
    unawaited(refresh());
  }

  Future<void> _present(ModuleResult result, String title) async {
    if (!mounted) return;
    await presentModuleResult(
      context,
      result,
      title: title,
      moduleId: widget.controller.moduleId,
      services: widget.services,
    );
  }

  Future<String?> _preview(ModuleViewAction preview) async {
    final result = await _top.peek(preview.command, preview.arguments);
    return result?.text;
  }

  Widget? _preference(BuildContext context, String id) {
    final module = _modules.installedModule(widget.controller.moduleId);
    final field = module?.preferenceFields
        .where((field) => field.id == id)
        .firstOrNull;
    if (module == null || field == null) return null;
    return _PreferenceItem(
      key: ValueKey('module-preference-${module.id}-$id'),
      modules: _modules,
      module: module,
      field: field,
      pickers: _pickers,
      enabled: widget.enabled,
    );
  }

  @override
  Widget build(BuildContext context) {
    final views = _top;
    return FailureToasts(
      listenable: views,
      failure: () => views.errorMessage,
      child: ListenableBuilder(
        listenable: Listenable.merge([views, _modules]),
        builder: (context, _) {
          final view = views.view;
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (_layers.isNotEmpty) ...[
                Align(
                  alignment: Alignment.centerLeft,
                  child: Button.ghost(
                    key: const ValueKey('module-view-back'),
                    style: AppTheme.actionButtonStyle(
                      const ButtonStyle.ghost(),
                    ),
                    leading: const AppIconFrame(
                      child: Icon(LucideIcons.arrowLeft),
                    ),
                    onPressed: () {
                      setState(() => _layers.removeLast().dispose());
                      unawaited(_top.refresh());
                    },
                    child: Text(context.l10n.commonBack),
                  ),
                ),
                const Gap(AppSpacing.md),
              ],
              if (view != null)
                ModuleViewSections(
                  key: ValueKey(
                    'module-view-${views.moduleId}-${_layers.length}',
                  ),
                  view: view,
                  enabled: widget.enabled && !views.busy,
                  onAction: (action) => unawaited(_act(action)),
                  onCopy: widget.services.clipboard.writeText,
                  preferenceBuilder: _preference,
                  formBuilder: (context, form) => ModuleViewFormCard(
                    key: ObjectKey(form),
                    form: form,
                    enabled: widget.enabled && !views.busy,
                    createDraft: (fields, values) =>
                        _modules.form(fields, values),
                    fieldBuilder: (draft, field) =>
                        _pickers.build(draft, field),
                    onSubmit: (action) => unawaited(_act(action)),
                    onPreview: _preview,
                  ),
                ),
            ],
          );
        },
      ),
    );
  }
}

/// Presents a command result a view does not replace: text in a dialog, a
/// message as a toast, a file saved where the user chooses, and a delivery
/// copied to the clipboard.
Future<void> presentModuleResult(
  BuildContext context,
  ModuleResult result, {
  required String title,
  required String moduleId,
  required ModuleViewServices services,
}) async {
  try {
    if (result.file case final file?) {
      final saver = services.fileSaver;
      if (saver != null) await saver.save(moduleId, file);
      return;
    }
    if (result.delivery case final delivery?) {
      final copier = services.delivery;
      if (copier == null) return;
      if (delivery.requireAuth &&
          !await verifyDeviceOwner(
            context,
            authenticator: services.authenticator,
          )) {
        return;
      }
      await copier.deliver(moduleId, delivery, paste: false);
      if (context.mounted) {
        AppToast.show(
          context,
          title: title,
          message: context.l10n.commonCopy,
          tone: AppToastTone.success,
        );
      }
      return;
    }
    if (result.results != null || result.text.trim().isEmpty) return;
    if (!context.mounted) return;
    if (result.message) {
      AppToast.show(
        context,
        title: title,
        message: result.text,
        tone: AppToastTone.success,
      );
    } else {
      await showModuleResultDialog(context, title: title, text: result.text);
    }
  } catch (error) {
    if (!context.mounted) return;
    AppToast.show(
      context,
      title: context.l10n.appActionFailed,
      message:
          ErrorText.reason(error) ??
          AppStrings.current.modulesOperationIncomplete,
      tone: AppToastTone.error,
    );
  }
}

/// A drawer showing one module view; a result that is not a view closes it.
class ModuleViewDrawer extends StatelessWidget {
  const ModuleViewDrawer({
    super.key,
    required this.title,
    required this.controller,
    required this.onFinished,
    this.services = const ModuleViewServices(),
  });

  final String title;
  final ModuleViewController controller;
  final ModuleViewServices services;
  final ValueChanged<ModuleResult> onFinished;

  @override
  Widget build(BuildContext context) => CallbackShortcuts(
    bindings: {
      const SingleActivator(LogicalKeyboardKey.escape): () =>
          unawaited(closeDrawer(context)),
    },
    child: ConstrainedBox(
      key: ValueKey('module-view-drawer-${controller.moduleId}'),
      constraints: AppOverlays.drawerContentConstraints(context),
      child: Scaffold(
        headers: [
          AppBar(
            title: Text(title, maxLines: 1, overflow: TextOverflow.ellipsis),
            trailing: [
              Tooltip(
                tooltip: (context) =>
                    TooltipContainer(child: Text(context.l10n.commonClose)),
                child: Semantics(
                  label: context.l10n.commonClose,
                  button: true,
                  child: Button.ghost(
                    key: const ValueKey('module-view-drawer-close'),
                    style: AppTheme.controlButtonStyle(
                      const ButtonStyle.ghostIcon(),
                    ),
                    onPressed: () => closeDrawer(context),
                    child: const AppIconFrame(child: Icon(LucideIcons.x)),
                  ),
                ),
              ),
            ],
          ),
          const Divider(),
        ],
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(AppSpacing.lg),
          child: ModuleViewHost(
            controller: controller,
            services: services,
            inDrawer: true,
            onFinished: onFinished,
          ),
        ),
      ),
    ),
  );
}

/// One module preference inside a view: a switch saves at once; other
/// fields save with their own button.
class _PreferenceItem extends StatefulWidget {
  const _PreferenceItem({
    super.key,
    required this.modules,
    required this.module,
    required this.field,
    required this.pickers,
    required this.enabled,
  });

  final ModulesController modules;
  final InstalledModule module;
  final DynamicFormField field;
  final ModulePickerFields pickers;
  final bool enabled;

  @override
  State<_PreferenceItem> createState() => _PreferenceItemState();
}

class _PreferenceItemState extends State<_PreferenceItem> {
  late DynamicFormDraft _draft = _createDraft();

  DynamicFormDraft _createDraft() => widget.modules.form(
    [widget.field],
    {widget.field.id: ?widget.module.preferences[widget.field.id]},
  );

  Object? get _stored =>
      widget.module.preferences[widget.field.id] ?? widget.field.defaultValue;

  @override
  void didUpdateWidget(_PreferenceItem oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.module.preferences[widget.field.id] !=
            widget.module.preferences[widget.field.id] ||
        oldWidget.field != widget.field) {
      unawaited(_draft.close());
      _draft = _createDraft();
    }
  }

  @override
  void dispose() {
    unawaited(_draft.close());
    super.dispose();
  }

  Future<void> _save(Object value) => widget.modules.setPreferences(
    widget.module.id,
    {...widget.module.preferences, widget.field.id: value},
  );

  @override
  Widget build(BuildContext context) {
    final enabled = widget.enabled && !widget.modules.busy;
    if (widget.field.kind == DynamicFormFieldKind.boolean) {
      return SettingRow(
        title: widget.field.title,
        trailing: AppSwitchFrame(
          child: Switch(
            value: _stored == true,
            enabled: enabled,
            onChanged: enabled ? (value) => unawaited(_save(value)) : null,
          ),
        ),
      );
    }
    return Padding(
      padding: AppTheme.settingsRowPadding,
      child: ListenableBuilder(
        listenable: _draft,
        builder: (context, _) {
          final value = _draft.values[widget.field.id];
          final changed = '$value' != '$_stored';
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              DynamicFormFields(
                draft: _draft,
                enabled: enabled,
                fieldBuilder: (context, field) =>
                    widget.pickers.build(_draft, field, enabled: enabled),
              ),
              if (changed)
                Align(
                  alignment: Alignment.centerRight,
                  child: Button.primary(
                    key: ValueKey('module-preference-save-${widget.field.id}'),
                    style: AppTheme.actionButtonStyle(
                      const ButtonStyle.primary(),
                    ),
                    onPressed: enabled && _draft.valid && value != null
                        ? () => unawaited(_save(value))
                        : null,
                    child: Text(context.l10n.commonSave),
                  ),
                ),
            ],
          );
        },
      ),
    );
  }
}

/// Opens [controller]'s view in a drawer; a result that is not a view
/// closes it and is presented. The controller is disposed afterwards.
Future<void> showModuleViewDrawer(
  BuildContext context, {
  required String title,
  required ModuleViewController controller,
  required ModuleViewServices services,
}) async {
  ModuleResult? finished;
  try {
    await showOverlay<void>(
      context,
      AppOverlays.drawerConfiguration(context),
      builder: (drawerContext) => ModuleViewDrawer(
        title: title,
        controller: controller,
        services: services,
        onFinished: (result) {
          finished = result;
          unawaited(closeDrawer(drawerContext));
        },
      ),
    ).future;
  } finally {
    controller.dispose();
  }
  if (finished case final result? when context.mounted) {
    await presentModuleResult(
      context,
      result,
      title: title,
      moduleId: controller.moduleId,
      services: services,
    );
  }
}
