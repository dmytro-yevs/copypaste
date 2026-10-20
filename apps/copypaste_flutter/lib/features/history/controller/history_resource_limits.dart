/// Allocation controls, not a claim of native process memory qualification.
/// Both History presentations use the same limits. Native release receipts
/// must account for their combined usage and decoded engine/GPU allocations.
abstract final class HistoryResourceLimits {
  // The active page and visible anchors are protected during eviction, so a
  // presentation can temporarily retain an extra page per visible region.
  static const residentPages = 3;
  static const pageRows = 50;
  static const residentSummaryBytes = 2 * 1024 * 1024;
  static const previewCacheBytes = 16 * 1024 * 1024;
  static const sourceIconCacheBytes = 4 * 1024 * 1024;
  static const retainedSourceIcons = 128;
}
