/** Platform decisions that change the visible product surface belong in one
 * place. Android WebView's user agent contains `Android`; browser tests can
 * pass a value explicitly without pretending to be Tauri. */
export function isAndroid(userAgent = navigator.userAgent): boolean {
  return /\bAndroid\b/i.test(userAgent);
}
