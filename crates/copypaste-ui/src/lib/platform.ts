export function isAndroidPlatform(): boolean {
  return typeof navigator !== "undefined" && /android/i.test(navigator.userAgent);
}
