/** Platform decisions that change the visible product surface belong here. */
export function isAndroid(userAgent = navigator.userAgent): boolean {
  return /\bAndroid\b/i.test(userAgent);
}

export function isAndroidPlatform(): boolean {
  return isAndroid();
}
