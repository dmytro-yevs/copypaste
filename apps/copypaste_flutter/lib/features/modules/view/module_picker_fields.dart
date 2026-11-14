import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/forms/dynamic_form_draft.dart';
import '../../applications/controller/application_directory.dart';
import '../../applications/repository/application_catalog.dart';
import '../../applications/view/application_picker.dart';
import '../controller/modules_controller.dart';
import '../models/module_models.dart';

/// Device and application pickers for module fields. One instance serves a
/// settings drawer and is released with it.
class ModulePickerFields {
  ModulePickerFields({
    required ModulesController controller,
    required ApplicationCatalog applications,
  }) : _controller = controller,
       _catalog = applications,
       _directory = ApplicationDirectory(applications);

  final ModulesController _controller;
  final ApplicationCatalog _catalog;
  final ApplicationDirectory _directory;
  Future<List<ModuleChoice>>? _devices;

  /// The control for [field] of [draft], or null for other field kinds.
  Widget? build(
    DynamicFormDraft draft,
    DynamicFormField field, {
    bool enabled = true,
  }) => switch (field.kind) {
    DynamicFormFieldKind.devices => _DeviceSetField(
      key: ValueKey('module-devices-${field.id}'),
      devices: _devices ??= _controller.deviceChoices(),
      selected: _ids(draft, field),
      enabled: enabled,
      onChanged: (ids) => draft.setValue(field.id, ids),
    ),
    DynamicFormFieldKind.applications => _ApplicationSetField(
      key: ValueKey('module-applications-${field.id}'),
      catalog: _catalog,
      directory: _directory,
      selected: _ids(draft, field),
      enabled: enabled,
      onChanged: (ids) => draft.setValue(field.id, ids),
    ),
    _ => null,
  };

  static List<String> _ids(DynamicFormDraft draft, DynamicFormField field) =>
      (draft.values[field.id] as List).cast<String>();
}

class _DeviceSetField extends StatelessWidget {
  const _DeviceSetField({
    super.key,
    required this.devices,
    required this.selected,
    required this.enabled,
    required this.onChanged,
  });

  final Future<List<ModuleChoice>> devices;
  final List<String> selected;
  final bool enabled;
  final ValueChanged<List<String>> onChanged;

  @override
  Widget build(BuildContext context) => FutureBuilder<List<ModuleChoice>>(
    future: devices,
    builder: (context, snapshot) {
      final known = {
        for (final device in snapshot.data ?? const <ModuleChoice>[])
          device.id: device.title,
      };
      String name(String id) => known[id] ?? context.l10n.modulesUnknownDevice;
      return MultiSelect<String>(
        enabled: enabled,
        value: selected,
        placeholder: Text(context.l10n.modulesChooseDevices),
        onChanged: (values) => onChanged((values ?? const <String>[]).toList()),
        itemBuilder: (context, id) => MultiSelectChip(
          value: id,
          style: AppTheme.selectChipStyle,
          child: Text(name(id), maxLines: 1, overflow: TextOverflow.ellipsis),
        ),
        popup: SelectPopup<String>(
          items: SelectItemList(
            children: [
              for (final id in {...known.keys, ...selected})
                SelectItemButton<String>(value: id, child: Text(name(id))),
            ],
          ),
        ).call,
      );
    },
  );
}

class _ApplicationSetField extends StatelessWidget {
  const _ApplicationSetField({
    super.key,
    required this.catalog,
    required this.directory,
    required this.selected,
    required this.enabled,
    required this.onChanged,
  });

  final ApplicationCatalog catalog;
  final ApplicationDirectory directory;
  final List<String> selected;
  final bool enabled;
  final ValueChanged<List<String>> onChanged;

  Future<void> _add(BuildContext context) async {
    final app = await showApplicationPicker(
      context,
      catalog: catalog,
      exclude: selected,
    );
    if (app == null) return;
    directory.remember(app);
    onChanged([...selected, app.id]);
  }

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      if (selected.isEmpty)
        Text(context.l10n.modulesNoApps).muted().small()
      else
        for (final id in selected)
          ApplicationIdentity(
            key: ValueKey('module-application-$id'),
            id: id,
            directory: directory,
            builder: (context, app, label) => Row(
              children: [
                Expanded(child: label.small()),
                Tooltip(
                  tooltip: (context) => TooltipContainer(
                    child: Text(context.l10n.modulesRemoveApp(app.name)),
                  ),
                  child: Button.ghost(
                    style: AppTheme.controlButtonStyle(
                      const ButtonStyle.ghostIcon(),
                    ),
                    onPressed: enabled
                        ? () => onChanged([
                            for (final other in selected)
                              if (other != id) other,
                          ])
                        : null,
                    child: Semantics(
                      label: context.l10n.modulesRemoveApp(app.name),
                      child: const AppIconFrame(child: Icon(LucideIcons.x)),
                    ),
                  ),
                ),
              ],
            ),
          ),
      const Gap(AppSpacing.sm),
      Align(
        alignment: AlignmentDirectional.centerStart,
        child: Button.secondary(
          key: const ValueKey('module-application-add'),
          onPressed: enabled ? () => _add(context) : null,
          leading: const AppIconFrame(child: Icon(LucideIcons.plus)),
          child: Text(context.l10n.modulesAddApp),
        ),
      ),
    ],
  );
}
