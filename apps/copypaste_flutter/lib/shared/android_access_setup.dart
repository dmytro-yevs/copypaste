import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../app/theme/app_theme.dart';
import '../app/theme/app_tokens.dart';
import '../l10n/l10n.dart';
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
    final l10n = context.l10n;
    if (!shizuku.supported && !setup.granted) {
      return Card(
        child: SetupSettingRow(
          icon: LucideIcons.info,
          title: l10n.sharedAndroidVersionRequired,
          description: l10n.sharedUseAdbTab,
        ),
      );
    }
    final ready = setup.granted;
    final title = ready
        ? l10n.sharedOneTimeAccessApplied
        : !shizuku.installed
        ? l10n.sharedInstallShizuku
        : !shizuku.running
        ? l10n.sharedPairAndStartShizuku
        : shizuku.permission
        ? setup.applyAccessLabel
        : l10n.sharedAllowCopyPaste;
    final description = ready
        ? l10n.sharedShizukuNoLongerNeeded
        : !shizuku.installed
        ? l10n.sharedGetShizukuDescription
        : !shizuku.running
        ? l10n.sharedShizukuWirelessDebugging
        : shizuku.permission
        ? setup.applyAccessDescription
        : l10n.sharedApproveInShizuku;
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
                      ? l10n.sharedGetShizuku
                      : !shizuku.running
                      ? l10n.sharedOpenShizuku
                      : shizuku.permission
                      ? setup.applyAccessLabel
                      : l10n.sharedAllowCopyPaste,
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
            Expanded(child: Text(context.l10n.sharedRunOnComputer)),
            const Gap(AppSpacing.sm),
            Tooltip(
              tooltip: (context) => Text(context.l10n.sharedCopyAllCommands),
              child: Button.ghost(
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.ghostIcon(),
                ),
                onPressed: widget.setup.adbCommands.isEmpty
                    ? null
                    : () async {
                        if (await widget.setup.onCopyCommands() && mounted) {
                          setState(() => copied = true);
                        }
                      },
                child: AppIconFrame(
                  child: Icon(
                    copied ? LucideIcons.copyCheck : LucideIcons.copy,
                  ),
                ),
              ),
            ),
          ],
        ),
        const Gap(AppSpacing.sm),
        Text(
          context.l10n.sharedEnableUsbDebugging,
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
