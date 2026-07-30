/**
 * A11Y-4 / AT-15 / AT-16 / INV-19 — the dialog contract.
 *
 * All of this behaviour is Radix's, which is exactly why it is here: manifest
 * §9.1 says replace v1's `useFocusTrap` + `Dialog` + ref-counted `scrollLock`
 * with the primitive and **keep A11Y-4 and INV-19 as tests**. These tests are
 * that. If someone swaps the primitive out, they fail.
 */
import { describe, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { useState } from "react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { withUser } from "@/test/harness";

function Fixture({ nested = false }: { nested?: boolean }) {
  const [inner, setInner] = useState(false);
  return (
    <Dialog>
      <DialogTrigger asChild>
        <Button>Open</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Outer dialog</DialogTitle>
          <DialogDescription>A description.</DialogDescription>
        </DialogHeader>
        <Button>First</Button>
        <DialogFooter>
          <Button onClick={() => setInner(true)}>Last</Button>
        </DialogFooter>
        {nested && (
          <Dialog open={inner} onOpenChange={setInner}>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Inner dialog</DialogTitle>
              </DialogHeader>
              <Button onClick={() => setInner(false)}>Close inner</Button>
            </DialogContent>
          </Dialog>
        )}
      </DialogContent>
    </Dialog>
  );
}

describe("the dialog focus contract (AT-15)", () => {
  it("is a named, described dialog, and hides the rest of the app from AT", async () => {
    const { user, container } = withUser(<Fixture />);
    await user.click(screen.getByRole("button", { name: "Open" }));

    const dialog = await screen.findByRole("dialog");
    expect(dialog.getAttribute("aria-labelledby")).toBeTruthy();
    expect(dialog.getAttribute("aria-describedby")).toBeTruthy();
    // Modality here is `aria-hidden` on everything outside the portal rather
    // than `aria-modal` on the panel — assert the mechanism that is actually
    // in use, or the test passes while nothing is modal.
    expect(container.getAttribute("aria-hidden")).toBe("true");
  });

  it("moves focus inside on open", async () => {
    const { user } = withUser(<Fixture />);
    await user.click(screen.getByRole("button", { name: "Open" }));
    const dialog = await screen.findByRole("dialog");
    await waitFor(() => expect(dialog.contains(document.activeElement)).toBe(true));
  });

  it("cycles Tab within the panel", async () => {
    const { user } = withUser(<Fixture />);
    await user.click(screen.getByRole("button", { name: "Open" }));
    const dialog = await screen.findByRole("dialog");

    for (let i = 0; i < 8; i++) {
      await user.tab();
      expect(dialog.contains(document.activeElement)).toBe(true);
    }
  });

  it("closes on Escape and restores focus to whatever opened it", async () => {
    const { user } = withUser(<Fixture />);
    const trigger = screen.getByRole("button", { name: "Open" });
    await user.click(trigger);
    await screen.findByRole("dialog");

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });
});

describe("body scroll lock is reference counted (AT-16 / INV-19)", () => {
  it("stays locked until the last stacked dialog closes", async () => {
    const { user } = withUser(<Fixture nested />);
    const locked = () => document.body.hasAttribute("data-scroll-locked");
    expect(locked()).toBe(false);

    await user.click(screen.getByRole("button", { name: "Open" }));
    await screen.findByRole("dialog");
    await user.click(screen.getByRole("button", { name: "Last" }));
    await screen.findByRole("dialog", { name: "Inner dialog" });
    expect(locked()).toBe(true);

    await user.click(screen.getByRole("button", { name: "Close inner" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Inner dialog" })).toBeNull(),
    );
    // The outer dialog is still open: the inner one closing must not restore
    // scrolling underneath it.
    expect(locked()).toBe(true);

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(locked()).toBe(false));
  });
});
