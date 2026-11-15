import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../devices/device_label.dart';
import '../controller/history_controller.dart';
import '../controller/history_search_suggestions.dart';
import '../presentation/history_clip_presentation.dart';
import '../presentation/source_app_label.dart';

/// Anchors the suggestion popover to the History search field [child] and
/// binds arrow keys, Tab, Enter and Escape while it is open. Rows accept taps.
class HistorySearchSuggestions extends StatefulWidget {
  const HistorySearchSuggestions({
    super.key,
    required this.controller,
    required this.searchController,
    required this.focusNode,
    required this.child,
  });

  final HistoryController controller;
  final TextEditingController searchController;
  final FocusNode focusNode;
  final Widget child;

  @override
  State<HistorySearchSuggestions> createState() =>
      _HistorySearchSuggestionsState();
}

class _HistorySearchSuggestionsState extends State<HistorySearchSuggestions> {
  final _popover = OverlayController();
  final _highlightedKey = GlobalKey();

  HistorySearchSuggestionsController get _suggestions =>
      widget.controller.searchSuggestions;

  @override
  void initState() {
    super.initState();
    widget.focusNode.addListener(_focusChanged);
    _suggestions.addListener(_sync);
    if (widget.focusNode.hasFocus) _focusChanged();
  }

  @override
  void didUpdateWidget(covariant HistorySearchSuggestions oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.focusNode != widget.focusNode) {
      oldWidget.focusNode.removeListener(_focusChanged);
      widget.focusNode.addListener(_focusChanged);
    }
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.searchSuggestions.removeListener(_sync);
      _suggestions.addListener(_sync);
    }
  }

  @override
  void dispose() {
    widget.focusNode.removeListener(_focusChanged);
    _suggestions.removeListener(_sync);
    _popover.dispose();
    super.dispose();
  }

  void _focusChanged() {
    if (widget.focusNode.hasFocus) {
      unawaited(
        _suggestions.load().then((_) {
          if (!mounted || !widget.focusNode.hasFocus) return;
          _suggestions.update(
            widget.searchController.text,
            widget.controller.window.query,
            widget.controller.window.facets,
          );
        }),
      );
      return;
    }
    unawaited(_suggestions.remember(widget.searchController.text));
    _suggestions.dismiss();
  }

  void _sync() {
    if (!mounted) return;
    setState(() {});
    final open =
        widget.focusNode.hasFocus && _suggestions.suggestions.isNotEmpty;
    if (!open) {
      if (_popover.hasOpenOverlay) _popover.close();
      return;
    }
    if (_suggestions.highlighted >= 0) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        final row = _highlightedKey.currentContext;
        if (row != null && row.mounted) {
          unawaited(Scrollable.ensureVisible(row));
        }
      });
    }
    if (_popover.hasOpenOverlay) return;
    final shown = _popover.show<void>(
      context,
      // The field stays put while typing, so no per-frame anchor tracking.
      AppOverlays.selectPopoverConfiguration(
        context,
        widthConstraint: PopoverConstraint.anchorFixedSize,
        modal: false,
        follow: false,
      ),
      builder: _popup,
      adaptive: false,
    );
    // A tap outside closes the popover itself; the next edit reopens it.
    unawaited(
      shown.whenComplete(() {
        if (mounted && !_popover.hasOpenOverlay) _suggestions.dismiss();
      }),
    );
  }

  void _accept(HistorySearchSuggestion suggestion) {
    final completion = _suggestions.complete(
      suggestion,
      widget.searchController.text,
      widget.controller.window.query,
    );
    widget.searchController.value = TextEditingValue(
      text: completion.search,
      selection: TextSelection.collapsed(offset: completion.search.length),
    );
    unawaited(widget.controller.window.updateQuery(completion.query));
  }

  void _acceptHighlighted({bool orFirst = false}) {
    final suggestion =
        _suggestions.highlightedSuggestion ??
        (orFirst && _suggestions.suggestions.isNotEmpty
            ? _suggestions.suggestions.first
            : null);
    if (suggestion != null) _accept(suggestion);
  }

  @override
  Widget build(BuildContext context) {
    final open = _suggestions.suggestions.isNotEmpty;
    return CallbackShortcuts(
      bindings: open
          ? {
              const SingleActivator(LogicalKeyboardKey.arrowDown): () =>
                  _suggestions.move(1),
              const SingleActivator(LogicalKeyboardKey.arrowUp): () =>
                  _suggestions.move(-1),
              const SingleActivator(LogicalKeyboardKey.tab): () =>
                  _acceptHighlighted(orFirst: true),
              if (_suggestions.highlighted >= 0)
                const SingleActivator(LogicalKeyboardKey.enter):
                    _acceptHighlighted,
              const SingleActivator(LogicalKeyboardKey.escape):
                  _suggestions.dismiss,
            }
          : const {},
      child: widget.child,
    );
  }

  Widget _popup(BuildContext context) => TextFieldTapRegion(
    child: ConstrainedBox(
      constraints: const BoxConstraints(
        maxHeight: AppOverlaySize.selectMaxHeight,
      ),
      child: SurfaceCard(
        padding: const EdgeInsets.all(AppSpacing.xs),
        child: ListenableBuilder(
          listenable: _suggestions,
          builder: (context, _) {
            final suggestions = _suggestions.suggestions;
            final hasRecent = suggestions.any(
              (suggestion) =>
                  suggestion.kind == HistorySearchSuggestionKind.recent,
            );
            return Semantics(
              container: true,
              label: context.l10n.historySearchSuggestions,
              child: SingleChildScrollView(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (var i = 0; i < suggestions.length; i++)
                      _row(context, suggestions[i], i),
                    if (hasRecent)
                      Button(
                        key: const ValueKey('history-search-clear-recent'),
                        style: AppTheme.clipboardMenuButtonStyle(),
                        alignment: AlignmentDirectional.centerStart,
                        leading: _icon(LucideIcons.eraser),
                        onPressed: () => unawaited(_suggestions.clearRecent()),
                        child: Text(
                          context.l10n.historySearchClearRecent,
                        ).muted(),
                      ),
                  ],
                ),
              ),
            );
          },
        ),
      ),
    ),
  );

  Widget _row(BuildContext context, HistorySearchSuggestion suggestion, int i) {
    final strings = context.l10n;
    final highlighted = i == _suggestions.highlighted;
    final (
      IconData? icon,
      Widget label,
      String category,
    ) = switch (suggestion.kind) {
      HistorySearchSuggestionKind.recent => (
        LucideIcons.history,
        Text(suggestion.label, maxLines: 1, overflow: TextOverflow.ellipsis),
        strings.historySearchSuggestionRecent,
      ),
      HistorySearchSuggestionKind.tag => (
        LucideIcons.tag,
        Text(suggestion.label, maxLines: 1, overflow: TextOverflow.ellipsis),
        strings.historySearchSuggestionTag,
      ),
      HistorySearchSuggestionKind.clipKind => (
        HistoryClipPresentation.icon(suggestion.clipKind!),
        Text(suggestion.label, maxLines: 1, overflow: TextOverflow.ellipsis),
        strings.historySearchSuggestionKind,
      ),
      HistorySearchSuggestionKind.sourceApp => (
        null,
        SourceAppLabel(
          name: suggestion.label,
          icon: widget.controller.media.requestSourceIcon(suggestion.iconId),
        ),
        strings.historySearchSuggestionApp,
      ),
      HistorySearchSuggestionKind.device => (
        null,
        DeviceLabel(
          name: suggestion.label,
          deviceClass: suggestion.deviceClass,
        ),
        strings.historySearchSuggestionDevice,
      ),
    };
    return Button(
      key: highlighted ? _highlightedKey : ValueKey(('suggestion', i)),
      style: AppTheme.clipboardMenuButtonStyle(selected: highlighted),
      alignment: AlignmentDirectional.centerStart,
      leading: icon == null ? null : _icon(icon),
      trailing: Text(
        category,
        style: AppTheme.clipboardMetadataTextStyle(context),
      ).muted(),
      onPressed: () => _accept(suggestion),
      child: label,
    );
  }

  Widget _icon(IconData icon) => AppIconFrame(
    size: AppIconSize.sm,
    child: Icon(icon, size: AppIconSize.sm),
  );
}
