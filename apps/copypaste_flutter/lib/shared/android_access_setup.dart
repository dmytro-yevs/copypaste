import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../app/theme/app_theme.dart';
import '../app/theme/app_tokens.dart';
import '../platform/android/android_shizuku_state.dart';
import 'setup_setting_row.dart';

/// One-time Android access shared by onboarding and optional modules.
class AndroidAccessSetup extends StatelessWidget {
  const AndroidAccessSetup({
    super.key,
    required this.methodIndex,
    required this.onMethodChanged,
    required this.shizuku,
    required this.granted,
    required this.busy,
    required this.adbCommands,
    required this.applyAccessLabel,
    required this.applyAccessDescription,
    required this.onOpenShizuku,
    required this.onApplyAccess,
    required this.onCopyCommands,
  });

  final int methodIndex;
  final ValueChanged<int> onMethodChanged;
  final AndroidShizukuState shizuku;
  final bool granted;
  final bool busy;
  final String adbCommands;
  final String applyAccessLabel;
  final String applyAccessDescription;
  final VoidCallback onOpenShizuku;
  final VoidCallback onApplyAccess;
  final Future<bool> Function() onCopyCommands;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      _methodTabs(context),
      const Gap(AppSpacing.lg),
      if (methodIndex == 0)
        _ShizukuSetup(setup: this)
      else
        _AdbSetup(setup: this),
    ],
  );

  Widget _methodTabs(BuildContext context) {
    final theme = Theme.of(context);
    final tabsTheme = ComponentTheme.maybeOf<TabsTheme>(context);
    final direction = Directionality.of(context);
    final style = DefaultTextStyle.of(
      context,
    ).style.merge(theme.typography.small).merge(theme.typography.medium);
    var labelWidth = 0.0;
    for (final label in ['Shizuku', 'ADB']) {
      final painter = TextPainter(
        text: TextSpan(text: label, style: style),
        textDirection: direction,
        textScaler: MediaQuery.textScalerOf(context),
        maxLines: 1,
      )..layout();
      if (painter.width > labelWidth) labelWidth = painter.width;
      painter.dispose();
    }
    final tabPadding = tabsTheme?.tabPadding ?? AppTheme.tabsTheme.tabPadding!;
    final containerPadding =
        tabsTheme?.containerPadding ?? AppTheme.tabsTheme.containerPadding!;
    final minimumWidth =
        (labelWidth.ceilToDouble() + tabPadding.resolve(direction).horizontal) *
            2 +
        containerPadding.resolve(direction).horizontal;
    return LayoutBuilder(
      builder: (context, constraints) => SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: SizedBox(
          width: constraints.maxWidth < minimumWidth
              ? minimumWidth
              : constraints.maxWidth,
          child: Tabs(
            index: methodIndex,
            expand: true,
            onChanged: (index) {
              if (!busy) onMethodChanged(index);
            },
            children: const [
              TabItem(child: Text('Shizuku', softWrap: false)),
              TabItem(child: Text('ADB', softWrap: false)),
            ],
          ),
        ),
      ),
    );
  }
}

class _ShizukuSetup extends StatelessWidget {
  const _ShizukuSetup({required this.setup});
  final AndroidAccessSetup setup;

  @override
  Widget build(BuildContext context) {
    final shizuku = setup.shizuku;
    if (!shizuku.supported && !setup.granted) {
      return const Card(
        child: SetupSettingRow(
          icon: LucideIcons.info,
          title: 'Android 11 or newer is required',
          description: 'Use the ADB tab on this device.',
        ),
      );
    }
    final ready = setup.granted;
    final title = ready
        ? 'One-time access applied'
        : !shizuku.installed
        ? 'Install Shizuku'
        : !shizuku.running
        ? 'Pair and start Shizuku'
        : shizuku.permission
        ? setup.applyAccessLabel
        : 'Allow CopyPaste';
    final description = ready
        ? 'Shizuku is no longer needed.'
        : !shizuku.installed
        ? 'Get Shizuku to apply one-time access.'
        : !shizuku.running
        ? 'Use Wireless debugging in Shizuku, then return.'
        : shizuku.permission
        ? setup.applyAccessDescription
        : 'Approve CopyPaste once in Shizuku.';
    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(title).medium(),
          const Gap(AppSpacing.xs),
          Text(description, style: Theme.of(context).typography.xSmall).muted(),
          if (!ready) ...[
            const Gap(AppSpacing.md),
            Align(
              alignment: Alignment.centerLeft,
              child: Button.primary(
                onPressed: setup.busy
                    ? null
                    : !shizuku.installed || !shizuku.running
                    ? setup.onOpenShizuku
                    : setup.onApplyAccess,
                child: Text(
                  !shizuku.installed
                      ? 'Get Shizuku'
                      : !shizuku.running
                      ? 'Open Shizuku'
                      : shizuku.permission
                      ? setup.applyAccessLabel
                      : 'Allow CopyPaste',
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _AdbSetup extends StatefulWidget {
  const _AdbSetup({required this.setup});
  final AndroidAccessSetup setup;

  @override
  State<_AdbSetup> createState() => _AdbSetupState();
}

class _AdbSetupState extends State<_AdbSetup> {
  bool copied = false;

  @override
  Widget build(BuildContext context) => Card(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            const Expanded(child: Text('Run on your computer')),
            const Gap(AppSpacing.sm),
            Tooltip(
              tooltip: (context) => const Text('Copy all commands'),
              child: Button.ghost(
                style: const ButtonStyle.ghostIcon(),
                onPressed: widget.setup.adbCommands.isEmpty
                    ? null
                    : () async {
                        if (await widget.setup.onCopyCommands() && mounted) {
                          setState(() => copied = true);
                        }
                      },
                child: Icon(copied ? LucideIcons.copyCheck : LucideIcons.copy),
              ),
            ),
          ],
        ),
        const Gap(AppSpacing.sm),
        Text(
          'Enable USB debugging and connect this phone.',
          style: Theme.of(context).typography.xSmall,
        ).muted(),
        const Gap(AppSpacing.md),
        SelectableText(
          widget.setup.adbCommands,
          style: Theme.of(
            context,
          ).typography.inlineCode.copyWith(fontWeight: FontWeight.normal),
        ),
      ],
    ),
  );
}
