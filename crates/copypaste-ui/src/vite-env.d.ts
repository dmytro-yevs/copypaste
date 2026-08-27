/// <reference types="vite/client" />

declare const __COPYPASTE_APP_VERSION__: string;

interface ImportMetaEnv {
  /** Set by `@vitejs/plugin-legacy`: true only in the nomodule build. */
  readonly LEGACY: boolean;
  /** Set only by the maintained Android build command. */
  readonly VITE_ANDROID_BUILD?: string;
}
