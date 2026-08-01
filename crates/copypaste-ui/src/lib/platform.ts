/** The Android WebView includes `Android` in its user agent. Keep this small
 * platform boundary at the UI edge: backend commands remain identical. */
export function isAndroidPlatform(userAgent = navigator.userAgent): boolean {
  return /\bAndroid\b/i.test(userAgent);
}
