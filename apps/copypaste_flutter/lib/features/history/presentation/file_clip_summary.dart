import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/shared/middle_ellipsis.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

/// A file clip as its name over a one-line, middle-ellipsized source path, or
/// a multi-file clip as its file count over its member names.
class FileClipSummary extends StatelessWidget {
  const FileClipSummary({super.key, required this.clip, this.pathKey});

  final HistoryClip clip;
  final Key? pathKey;

  /// Member names shown before the remaining count.
  static const visibleMemberNames = 3;

  @override
  Widget build(BuildContext context) {
    final file = clip.file;
    if (file != null && file.isSet) return _buildSet(context, file);
    final reference = clip.file?.sourceReference;
    final path = reference?.trim().isNotEmpty == true
        ? reference!
        : clip.preview.contains(RegExp(r'[/\\]'))
        ? clip.preview
        : '';
    final name = clip.file?.name?.trim();
    final filename = name?.isNotEmpty == true
        ? name!
        : (path.isEmpty ? clip.preview : path).split(RegExp(r'[/\\]')).last;
    final theme = Theme.of(context);
    final pathStyle = theme.typography.xSmall.copyWith(
      color: theme.colorScheme.mutedForeground,
    );
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          filename.isEmpty ? context.l10n.quickPasteFileFallbackName : filename,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
        LayoutBuilder(
          builder: (context, constraints) => Text(
            path.isEmpty
                ? context.l10n.quickPastePathUnavailable
                : middleEllipsis(
                    path,
                    maxWidth: constraints.maxWidth,
                    style: pathStyle,
                    direction: Directionality.of(context),
                    textScaler: MediaQuery.textScalerOf(context),
                  ),
            key: pathKey,
            maxLines: 1,
            softWrap: false,
            style: pathStyle,
          ),
        ),
      ],
    );
  }

  Widget _buildSet(BuildContext context, HistoryFileDetails file) {
    final theme = Theme.of(context);
    final namesStyle = theme.typography.xSmall.copyWith(
      color: theme.colorScheme.mutedForeground,
    );
    final names = file.files.isEmpty
        ? [if (file.name?.trim().isNotEmpty == true) file.name!.trim()]
        : [for (final member in file.files) member.name];
    final remaining = file.fileCount! - names.take(visibleMemberNames).length;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          context.l10n.fileSetCount(file.fileCount!),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
        Row(
          key: pathKey,
          children: [
            Flexible(
              child: Text(
                names.take(visibleMemberNames).join(', '),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                softWrap: false,
                style: namesStyle,
              ),
            ),
            if (names.isNotEmpty && remaining > 0)
              Text(', +$remaining', maxLines: 1, style: namesStyle),
          ],
        ),
      ],
    );
  }
}
