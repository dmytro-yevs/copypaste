import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../history/presentation/source_app_label.dart';
import '../controller/application_directory.dart';
import '../models/application_models.dart';
import '../repository/application_catalog.dart';

/// A searchable list of installed and seen applications. Resolves to the
/// chosen one, or null when dismissed; apps in [exclude] are not offered.
/// The list loads when the picker opens and is released when it closes.
Future<ApplicationChoice?> showApplicationPicker(
  BuildContext context, {
  required ApplicationCatalog catalog,
  Iterable<String> exclude = const [],
}) => AppOverlays.showDialog<ApplicationChoice>(
  context,
  builder: (context) =>
      _ApplicationPicker(catalog: catalog, exclude: exclude.toList()),
);

class _ApplicationPicker extends StatefulWidget {
  const _ApplicationPicker({required this.catalog, required this.exclude});

  final ApplicationCatalog catalog;
  final List<String> exclude;

  @override
  State<_ApplicationPicker> createState() => _ApplicationPickerState();
}

class _ApplicationPickerState extends State<_ApplicationPicker> {
  late final _directory = ApplicationDirectory(widget.catalog);
  late final _candidates = _directory.candidates(exclude: widget.exclude);

  @override
  void dispose() {
    widget.catalog.cancelIcons();
    super.dispose();
  }

  Stream<List<Widget>> _results(String? query) async* {
    final needle = (query ?? '').trim().toLowerCase();
    final apps = await _candidates;
    yield [
      for (final app in apps)
        if (needle.isEmpty ||
            app.name.toLowerCase().contains(needle) ||
            app.id.toLowerCase().contains(needle))
          CommandItem(
            key: ValueKey('application-${app.id}'),
            // Built per visible row, so icons load only as rows appear.
            title: Builder(
              builder: (context) =>
                  SourceAppLabel(name: app.name, icon: _directory.icon(app)),
            ),
            onTap: () => Navigator.pop(context, app),
          ),
    ];
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): () =>
            Navigator.maybePop(context),
      },
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxHeight:
              MediaQuery.sizeOf(context).height *
              AppOverlaySize.dialogContentHeightFactor,
        ),
        child: AppTheme.commandSurface(
          context,
          child: Command(
            key: const ValueKey('application-picker'),
            debounceDuration: Duration.zero,
            searchPlaceholder: Text(l10n.applicationPickerSearch),
            emptyBuilder: (context) => StateView.empty(
              compact: true,
              icon: LucideIcons.searchX,
              title: l10n.applicationPickerEmpty,
            ),
            builder: (context, query) => _results(query),
          ),
        ),
      ),
    );
  }
}

/// A stored application identifier shown as its application: the name and
/// icon when it is installed or seen here, else the identifier itself with
/// the [SourceAppLabel] initials fallback.
class ApplicationIdentity extends StatelessWidget {
  const ApplicationIdentity({
    super.key,
    required this.id,
    required this.directory,
    required this.builder,
  });

  final String id;
  final ApplicationDirectory directory;
  final Widget Function(
    BuildContext context,
    ApplicationChoice app,
    Widget label,
  )
  builder;

  @override
  Widget build(BuildContext context) => FutureBuilder<ApplicationChoice?>(
    future: directory.describe(id),
    builder: (context, snapshot) {
      final resolved = snapshot.connectionState == ConnectionState.done;
      final app = snapshot.data ?? ApplicationChoice(id: id, name: id);
      return builder(
        context,
        app,
        SourceAppLabel(
          name: app.name,
          icon: resolved ? directory.icon(app) : Future.value(),
        ),
      );
    },
  );
}
