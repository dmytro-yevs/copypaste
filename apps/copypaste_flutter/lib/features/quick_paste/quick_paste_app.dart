import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_theme.dart';
import 'quick_paste_controller.dart';
import 'quick_paste_view.dart';

class QuickPasteApp extends StatefulWidget {
  const QuickPasteApp({super.key, required this.controller});

  final QuickPasteController controller;

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
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ShadcnApp(
      title: 'CopyPaste Quick Paste',
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: AppTheme.mode,
      builder: AppTheme.builder,
      localizationsDelegates: GlobalMaterialLocalizations.delegates,
      supportedLocales: ShadcnLocalizations.supportedLocales,
      home: QuickPasteView(controller: widget.controller),
    );
  }
}
