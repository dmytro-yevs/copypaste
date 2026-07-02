import { EmptyState } from "../../../components/EmptyState";
import { RestartDaemonButton } from "../../../components/RestartDaemonButton";

// Task 6.7: "empty states (all 4 documented variants)". design.md's component
// inventory (`EmptyState.tsx` entry) clarifies EmptyState is ONE component API
// used across 7 app contexts, but "the '4 states' refers to the Popup only" —
// offline, starting-up, no-matches, nothing-copied-yet (copy verbatim from
// popup/Popup.tsx's four EmptyState call sites).
export function EmptyStatesSection() {
  return (
    <section id="gallery-empty-states">
      <h2>Empty states — the four documented Popup variants</h2>
      <div className="gallery__col">
        <EmptyState
          title="Clipboard service offline"
          body="The background service is not running. Restart it from Settings."
          action={<RestartDaemonButton onRestarted={() => {}} />}
        />
        <EmptyState
          title="Starting up…"
          body="The clipboard service is initialising. It will be ready in a moment."
        />
        <EmptyState title='No matches for "clip"' body="Try a different search term." />
        <EmptyState title="Nothing copied yet" body="Copy something and it will appear here." />
      </div>
    </section>
  );
}
