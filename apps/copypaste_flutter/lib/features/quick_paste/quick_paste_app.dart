import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/localization/app_language.dart';
import '../../app/theme/app_appearance.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_window_material.dart';
import '../../app/window_material_controller.dart';
import '../../l10n/l10n.dart';
import '../../platform/desktop/desktop_shortcuts.dart';
import 'quick_paste_controller.dart';
import 'quick_paste_view.dart';

class QuickPasteApp extends StatefulWidget {
  const QuickPasteApp({
    super.key,
    required this.controller,
    this.language,
    this.windowMaterial,
    this.appearance,
  });

  final QuickPasteController controller;
  final AppLanguageController? language;
  final WindowMaterialController? windowMaterial;

  /// The preferred theme mode and accent; the defaults without one.
  final AppAppearance? appearance;

  @override
  State<QuickPasteApp> createState() => _QuickPasteAppState();
}

class _QuickPasteAppState extends State<QuickPasteApp> {
  @override
  void initState() {
    super.initState();
    widget.controller.initialize();
  }

  @override
  void dispose() {
    widget.controller.dispose();
    widget.windowMaterial?.dispose();
    widget.appearance?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AppTheme.withAppearance(
      widget.appearance,
      (themeMode, light, dark) => ShadcnApp(
        title: 'CopyPaste Quick Paste',
        debugShowCheckedModeBanner: false,
        theme: light,
        darkTheme: dark,
        themeMode: themeMode,
        builder: (context, child) {
          final content = AppTheme.builder(
            context,
            Shortcuts(
              shortcuts: desktopTextEditingShortcuts,
              child: child ?? const SizedBox.shrink(),
            ),
          );
          final material = widget.windowMaterial;
          if (material == null) return content;
          return AppWindowMaterial(state: material, child: content);
        },
        locale: widget.language?.locale,
        localizationsDelegates: AppLocalization.delegates,
        supportedLocales: AppLocalization.supportedLocales,
        localeListResolutionCallback: AppLocalization.resolve,
        home: AppStringsLocale(
          child: QuickPasteView(controller: widget.controller),
        ),
      ),
    );
  }
}
