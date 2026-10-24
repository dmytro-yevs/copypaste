import 'dart:async';

import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_theme.dart';
import '../../platform/desktop/desktop_shortcuts.dart';
import '../../platform/desktop/quick_paste_host.dart';
import 'quick_paste_controller.dart';
import 'quick_paste_view.dart';

class QuickPasteApp extends StatefulWidget {
  const QuickPasteApp({super.key, required this.controller})
    : createController = null,
      host = null;

  const QuickPasteApp.retained({
    super.key,
    required this.createController,
    required this.host,
  }) : controller = null;

  final QuickPasteController? controller;
  final QuickPasteController Function()? createController;
  final RetainedQuickPasteContextHost? host;

  @override
  State<QuickPasteApp> createState() => _QuickPasteAppState();
}

class _QuickPasteAppState extends State<QuickPasteApp> {
  QuickPasteController? _controller;
  int? _presentationId;
  int _epoch = 0;
  bool _visible = false;
  Future<void> _release = Future<void>.value();

  @override
  void initState() {
    super.initState();
    _controller = widget.controller;
    final host = widget.host;
    if (host == null) {
      _visible = true;
      _controller!.initialize();
    } else {
      host.setOpenedHandler(_opened);
      host.setHiddenHandler(_hidden);
      host.setShutdownHandler(_shutdown);
      unawaited(host.signalReady());
    }
  }

  Future<void> _releaseController(QuickPasteController? controller) {
    if (controller == null) return _release;
    final shutdown = controller.shutdown();
    return _release = Future.wait([_release, shutdown]).then<void>((_) {});
  }

  void _unmountPresentation() {
    if (!mounted) return;
    setState(() => _visible = false);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && !_visible) {
        PaintingBinding.instance.imageCache.clear();
        PaintingBinding.instance.imageCache.clearLiveImages();
      }
    });
    WidgetsBinding.instance.scheduleWarmUpFrame();
  }

  Future<void> _opened(int id, {required bool inspectorVisible}) async {
    final epoch = ++_epoch;
    _presentationId = id;
    final previous = _controller;
    _controller = null;
    _unmountPresentation();
    await _releaseController(previous);
    if (!mounted || epoch != _epoch) return;
    final controller = widget.createController!();
    assert(!controller.ownsHostLifecycle);
    _controller = controller;
    await controller.initialize();
    if (!mounted || epoch != _epoch) return;
    await controller.opened(id, inspectorVisible: inspectorVisible);
    if (!mounted || epoch != _epoch) return;
    setState(() => _visible = true);
    WidgetsBinding.instance.scheduleWarmUpFrame();
  }

  Future<void> _hidden(int id) async {
    if (_presentationId != id) return;
    _presentationId = null;
    ++_epoch;
    final controller = _controller;
    _controller = null;
    _unmountPresentation();
    await _releaseController(controller);
  }

  Future<void> _shutdown() async {
    _presentationId = null;
    ++_epoch;
    final controller = _controller;
    _controller = null;
    _unmountPresentation();
    await _releaseController(controller);
  }

  @override
  void dispose() {
    if (widget.host case final host?) {
      host.setOpenedHandler(null);
      host.setHiddenHandler(null);
      host.setShutdownHandler(null);
      unawaited(_releaseController(_controller));
      unawaited(host.dispose());
    } else {
      _controller?.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ShadcnApp(
      title: 'CopyPaste Quick Paste',
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: AppTheme.mode,
      builder: (context, child) => AppTheme.builder(
        context,
        Shortcuts(
          shortcuts: desktopTextEditingShortcuts,
          child: child ?? const SizedBox.shrink(),
        ),
      ),
      localizationsDelegates: GlobalMaterialLocalizations.delegates,
      supportedLocales: ShadcnLocalizations.supportedLocales,
      home: _visible && _controller != null
          ? QuickPasteView(key: ValueKey(_epoch), controller: _controller!)
          : const SizedBox.expand(),
    );
  }
}
