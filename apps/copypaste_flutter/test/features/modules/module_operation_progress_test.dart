import 'dart:async';

import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/modules_repository.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/rendering.dart' show RenderParagraph;
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'modules_test_support.dart';

const _megabyte = 1024 * 1024;
final _install = find.byKey(ValueKey('module-install-${testModule.id}'));
final _card = find.byKey(ValueKey('module-${testModule.id}'));
final _progress = find.byKey(ValueKey('module-progress-${testModule.id}'));

void main() {
  testWidgets(
    'a minimum-width card at text scale 2.0 keeps its action in place through every phase',
    (tester) async {
      final semantics = tester.ensureSemantics();
      final repository = _ControlledRepository();
      final marketplace = _ControlledMarketplace();
      await _pump(
        tester,
        repository: repository,
        marketplace: marketplace,
        theme: AppTheme.light,
        locale: const Locale('uk'),
        scale: 2,
      );
      final cardSize = tester.getSize(_card);
      final actionHeight = tester.getSize(_install).height;
      final idleInset =
          tester.getBottomRight(_card).dy - tester.getBottomRight(_install).dy;
      // The progress takes the install action's place without moving it.
      void expectActionInPlace(Finder action) {
        expect(tester.getSize(_card), cardSize);
        expect(tester.getSize(action).height, actionHeight);
        expect(
          tester.getBottomRight(_card).dy - tester.getBottomRight(action).dy,
          idleInset,
        );
      }

      await tester.ensureVisible(_install);
      await tester.tap(_install);
      await tester.pump();
      marketplace.report!(0.42);
      await tester.pump();
      expect(_install, findsNothing);
      expect(_progress, findsOneWidget);
      expect(_text('Завантаження · 42%').hitTestable(), findsOneWidget);
      _expectSingleLines(tester);
      expectActionInPlace(_progress);
      expect(
        tester.getSize(find.byKey(const ValueKey('module-module-1'))),
        cardSize,
      );
      var node = tester.getSemantics(_progress).getSemanticsData();
      expect(node.label, contains('Встановлення модуля Text Tools'));
      expect(node.value, '42%');

      marketplace.package.complete(
        SelectedModulePackage(
          path: '/private/test.cpmodule',
          dispose: () async {},
        ),
      );
      await tester.pump();
      await tester.pump();
      expect(_text('Встановлення').hitTestable(), findsOneWidget);
      expect(
        find.descendant(of: _progress, matching: find.textContaining('%')),
        findsNothing,
      );
      _expectSingleLines(tester);
      expectActionInPlace(_progress);
      node = tester.getSemantics(_progress).getSemanticsData();
      expect(node.label, contains('Встановлення модуля Text Tools'));
      expect(node.value, isEmpty);

      repository.installation.completeError(
        const ModulesException('Invalid signature.'),
      );
      await tester.pumpAndSettle();
      expect(tester.binding.hasScheduledFrame, isFalse);
      // A failure is a toast; the card offers the install action again.
      expect(_progress, findsNothing);
      expect(find.text('Invalid signature.'), findsOneWidget);
      expect(tester.widget<Button>(_install).onPressed, isNotNull);
      expectActionInPlace(_install);
      expect(tester.takeException(), isNull);
      // Let the failure toast expire before the tree is disposed.
      await tester.pump(const Duration(minutes: 1));
      await tester.pumpAndSettle();
      semantics.dispose();
    },
  );

  testWidgets('a card names every phase when the names fit', (tester) async {
    final marketplace = _ControlledMarketplace();
    await _pump(
      tester,
      repository: _ControlledRepository(),
      marketplace: marketplace,
      theme: AppTheme.dark,
      locale: const Locale('en'),
      scale: 1,
    );
    await tester.tap(_install);
    await tester.pump();
    marketplace.report!(0.42);
    await tester.pump();
    expect(_text('Download · 42%'), findsOneWidget);
    marketplace.package.complete(
      SelectedModulePackage(
        path: '/private/test.cpmodule',
        dispose: () async {},
      ),
    );
    await tester.pump();
    await tester.pump();
    expect(_text('Install'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('no step name wraps on a minimum-width card at text scale 2.0', (
    tester,
  ) async {
    for (final locale in ['en', 'de', 'uk', 'pl']) {
      final repository = _ControlledRepository();
      final marketplace = _ControlledMarketplace();
      await _pump(
        tester,
        repository: repository,
        marketplace: marketplace,
        theme: AppTheme.light,
        locale: Locale(locale),
        scale: 2,
      );
      await tester.ensureVisible(_install);
      final actionBottom = tester.getBottomRight(_install).dy;
      await tester.tap(_install);
      await tester.pump();
      marketplace.report!(0.42);
      await tester.pump();
      _expectSingleLines(tester, reason: locale);
      expect(tester.getBottomRight(_progress).dy, actionBottom, reason: locale);
      marketplace.package.complete(
        SelectedModulePackage(
          path: '/private/test.cpmodule',
          dispose: () async {},
        ),
      );
      await tester.pump();
      await tester.pump();
      _expectSingleLines(tester, reason: locale);
      expect(tester.getBottomRight(_progress).dy, actionBottom, reason: locale);
      repository.installation.complete();
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull, reason: locale);
    }
  });
}

/// Every text in the progress action renders on one line at its width.
void _expectSingleLines(WidgetTester tester, {String? reason}) {
  final paragraphs = tester.renderObjectList<RenderParagraph>(
    find.descendant(of: _progress, matching: find.byType(RichText)),
  );
  for (final paragraph in paragraphs) {
    final painter = TextPainter(
      text: paragraph.text,
      textDirection: TextDirection.ltr,
      textScaler: paragraph.textScaler,
    )..layout(maxWidth: paragraph.size.width + 0.01);
    expect(
      painter.computeLineMetrics(),
      hasLength(1),
      reason: '$reason: ${paragraph.text.toPlainText()}',
    );
    painter.dispose();
  }
}

Finder _text(String text) =>
    find.descendant(of: _progress, matching: find.text(text));

Future<void> _pump(
  WidgetTester tester, {
  required ModulesRepository repository,
  required _ControlledMarketplace marketplace,
  required ThemeData theme,
  required Locale locale,
  required double scale,
}) async {
  await tester.binding.setSurfaceSize(
    const Size(AppLayoutSize.marketplaceCardMinWidth * 2 + AppSpacing.md, 2400),
  );
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final controller = ModulesController(
    repository: repository,
    marketplace: marketplace,
  );
  addTearDown(controller.dispose);
  await controller.initialize();
  await tester.pumpWidget(
    ShadcnApp(
      theme: theme,
      locale: locale,
      localizationsDelegates: AppLocalization.delegates,
      supportedLocales: AppLocalization.supportedLocales,
      builder: (context, child) => AppTheme.builder(
        context,
        MediaQuery(
          data: MediaQuery.of(
            context,
          ).copyWith(textScaler: TextScaler.linear(scale)),
          child: child!,
        ),
      ),
      home: Scaffold(
        child: SingleChildScrollView(
          child: ModulesSettingsView(controller: controller),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

class _ControlledMarketplace extends MemoryModuleMarketplace {
  _ControlledMarketplace() {
    modules = [
      for (final id in [testModule.id, 'module-1'])
        MarketplaceModule(
          id: id,
          title: testModule.title,
          description: testModule.description,
          version: testMarketplaceModule.version,
          supportedPlatforms: ModulePlatform.values,
          artifact: ModuleArtifact(
            downloadUri: testMarketplaceModule.artifact!.downloadUri,
            sizeBytes: 10 * _megabyte,
            sha256: '0' * 64,
          ),
        ),
    ];
  }

  final package = Completer<SelectedModulePackage>();
  void Function(double)? report;

  @override
  Future<SelectedModulePackage> download(
    MarketplaceModule module, {
    required void Function(double) onProgress,
  }) {
    report = onProgress;
    return package.future;
  }
}

class _ControlledRepository extends MemoryModulesRepository {
  final installation = Completer<void>();
  @override
  Future<void> install(String packagePath) => installation.future;
}
