export const EXTRA_SMALL_MAX_PX = 479;
export const FAILURE_MAX_PX = 599;
export const SMALL_MAX_PX = 639;
export const EXPANDED_MIN_PX = 640;
export const MEDIUM_MAX_PX = 719;
export const TOOLBAR_MAX_PX = 759;
export const WIDE_MIN_PX = 760;
export const ROSTER_MIN_PX = 761;
export const HISTORY_INSPECTOR_MIN_PX = 900;
export const ROSTER_WIDE_MIN_PX = 981;
export const INSPECTOR_SHORT_HEIGHT_MAX_PX = 520;
export const SHORT_HEIGHT_MAX_PX = 620;
export const SHORT_MOBILE_HEIGHT_MAX_PX = 720;

export const RESPONSIVE_POSTCSS_VARIABLES = {
  cpXs: `${EXTRA_SMALL_MAX_PX}px`,
  cpFailure: `${FAILURE_MAX_PX}px`,
  cpSm: `${SMALL_MAX_PX}px`,
  cpLg: `${EXPANDED_MIN_PX}px`,
  cpMd: `${MEDIUM_MAX_PX}px`,
  cpToolbar: `${TOOLBAR_MAX_PX}px`,
  cpWide: `${WIDE_MIN_PX}px`,
  cpRoster: `${ROSTER_MIN_PX}px`,
  cpRosterWide: `${ROSTER_WIDE_MIN_PX}px`,
  cpInspectorShort: `${INSPECTOR_SHORT_HEIGHT_MAX_PX}px`,
  cpShort: `${SHORT_HEIGHT_MAX_PX}px`,
  cpShortMobile: `${SHORT_MOBILE_HEIGHT_MAX_PX}px`,
} as const satisfies Record<string, `${number}px`>;

export const EXPANDED_QUERY = `(min-width: ${EXPANDED_MIN_PX}px)`;
