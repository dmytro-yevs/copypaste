// CloudAccountMismatchBanner.tsx — CopyPaste-1jms.34
//
// Renders a warning banner in the SyncTab cloud section when the local
// Supabase account identity is available AND differs from a paired peer's
// identity.
//
// NOTE (CopyPaste-1jms.35 DEFERRED): peer supabase_account_id is not yet
// plumbed through the `list_peers` response, so full cross-device mismatch
// detection is not yet possible.  This component accepts an explicit
// `hasMismatch` boolean computed by the caller.  When the caller has no peer
// account ids to compare, it passes `hasMismatch={false}` and the banner is
// hidden — no false positives.  The follow-up issue CopyPaste-1jms.35 tracks
// the peer-account-id plumbing needed to enable actual detection.

import { AlertTriangle } from "lucide-react";

export interface CloudAccountMismatchBannerProps {
  /**
   * True when the local account id differs from at least one paired peer's
   * account id. The caller is responsible for the comparison; this component
   * only renders the banner when this is true.
   *
   * Until peer account ids are plumbed (CopyPaste-1jms.35), callers should
   * always pass `false` here.
   */
  hasMismatch: boolean;
  /**
   * The local device's Supabase account id (for informational display).
   * Optional — omitted/null when cloud-sync is off or anon-key-only.
   */
  localAccountId?: string | null;
}

/**
 * Warning banner shown in the cloud section of SyncTab when two paired
 * devices are configured with different Supabase accounts.
 *
 * When `hasMismatch` is false (the default until CopyPaste-1jms.35 is
 * implemented) the banner is not rendered — do not show speculative warnings.
 */
export function CloudAccountMismatchBanner({
  hasMismatch,
  localAccountId,
}: CloudAccountMismatchBannerProps) {
  if (!hasMismatch) return null;

  return (
    <div
      role="alert"
      data-testid="cloud-account-mismatch-banner"
      className="banner banner--warn"
    >
      <AlertTriangle aria-hidden="true" />
      <span className="banner__x">
        <b>Supabase account mismatch detected.</b> Two or more paired
        devices are using different Supabase accounts or projects. Clipboard items
        will not sync — Supabase RLS only allows rows owned by the same GoTrue
        user to be shared. Make sure every device signs in with the same Supabase
        email and points to the same Supabase project URL.
        {localAccountId != null && (
          <span>
            {" "}This device: <code>{localAccountId}</code>
          </span>
        )}
      </span>
    </div>
  );
}
