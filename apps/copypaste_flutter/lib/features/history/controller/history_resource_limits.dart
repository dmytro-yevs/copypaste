/// Allocation controls, not a claim of native process memory qualification.
/// Both History presentations use the same limits. Native release receipts
/// must account for their combined usage and decoded engine/GPU allocations.
abstract final class HistoryResourceLimits {
  static const previewCacheBytes = 16 * 1024 * 1024;
  static const sourceIconCacheBytes = 4 * 1024 * 1024;
  static const retainedSourceIcons = 128;
}
