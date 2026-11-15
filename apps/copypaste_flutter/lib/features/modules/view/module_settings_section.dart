import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../settings/view/settings_section.dart';
import '../controller/module_view_controller.dart';
import '../controller/module_view_services.dart';
import '../controller/modules_controller.dart';
import '../models/module_models.dart';
import 'module_icons.dart';
import 'module_view_host.dart';

/// The icon of [module]'s Settings section.
IconData moduleSettingsIcon(InstalledModule module) =>
    module.surfaces.settings?.icon?.data ?? LucideIcons.puzzle;

/// The top-level Settings section of a module with a settings surface: the
/// view its settings command returns. Key it by the module.
class ModuleSettingsSection extends StatefulWidget {
  const ModuleSettingsSection({
    super.key,
    required this.modules,
    required this.module,
    this.services = const ModuleViewServices(),
    this.showHeading = true,
  });

  final ModulesController modules;
  final InstalledModule module;
  final ModuleViewServices services;
  final bool showHeading;

  @override
  State<ModuleSettingsSection> createState() => _ModuleSettingsSectionState();
}

class _ModuleSettingsSectionState extends State<ModuleSettingsSection> {
  late final _views = ModuleViewController(
    modules: widget.modules,
    moduleId: widget.module.id,
  );

  String get _command => widget.module.surfaces.settings!.command;

  @override
  void initState() {
    super.initState();
    unawaited(_views.open(_command));
    widget.modules.addListener(_takeDrawerRequest);
    WidgetsBinding.instance.addPostFrameCallback((_) => _takeDrawerRequest());
  }

  @override
  void dispose() {
    widget.modules.removeListener(_takeDrawerRequest);
    _views.dispose();
    super.dispose();
  }

  void _takeDrawerRequest() {
    if (!mounted) return;
    final request = widget.modules.takeDrawerRequest(widget.module.id);
    if (request != null) unawaited(_openDrawer(request));
  }

  Future<void> _openDrawer(ModuleDrawerRequest request) async {
    final result = await _views.run(request.command, const {}, show: false);
    if (!mounted || result == null) return;
    final view = result.view;
    if (view == null) {
      await presentModuleResult(
        context,
        result,
        title: request.title,
        moduleId: widget.module.id,
        services: widget.services,
      );
      return;
    }
    await showModuleViewDrawer(
      context,
      title: request.title,
      controller: ModuleViewController(
        modules: widget.modules,
        moduleId: widget.module.id,
        view: view,
        source: (request.command, const {}),
        transient: true,
      ),
      services: widget.services,
    );
    if (mounted) unawaited(_views.refresh());
  }

  @override
  Widget build(BuildContext context) => SettingsSection(
    title: widget.module.title,
    description: widget.module.description,
    icon: moduleSettingsIcon(widget.module),
    showHeading: widget.showHeading,
    groupContent: false,
    children: [
      ListenableBuilder(
        listenable: _views,
        builder: (context, _) {
          if (_views.view != null) {
            return ModuleViewHost(
              key: ValueKey('module-settings-view-${widget.module.id}'),
              controller: _views,
              services: widget.services,
            );
          }
          if (_views.busy) return const StateView.loading();
          return StateView.error(
            title: context.l10n.modulesOperationFailed,
            message: _views.errorMessage,
            onRetry: () => unawaited(_views.open(_command)),
          );
        },
      ),
    ],
  );
}
