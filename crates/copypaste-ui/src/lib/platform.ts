/** Platform decisions that change the visible product surface belong here. */
export function isAndroid(userAgent = navigator.userAgent): boolean {
  return /\bAndroid\b/i.test(userAgent);
}

export function isAndroidPlatform(): boolean {
  return isAndroid();
}

/**
 * Android reports `Linux; Android`, so the Android test has to win. Without
 * that order a phone would be told to hold Ctrl.
 */
export function isWindows(userAgent = navigator.userAgent): boolean {
  return !isAndroid(userAgent) && /\bWindows\b/i.test(userAgent);
}

export function isWindowsPlatform(): boolean {
  return isWindows();
}
