/**
 * Two versions and two links.
 *
 * The app and the background service update separately and can disagree, so a
 * panel that reports one number is a panel that cannot answer "which build is
 * this?" — the question About exists for (ui-parity 8, B-21).
 *
 * The links are asserted by `href` and by `target`, which is all a jsdom test
 * can see. Whether either one reaches a browser on macOS or Android is
 * NOT VERIFIED IN CI, and cannot be until an external opener is registered.
 */
import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen } from "@testing-library/react";

import { AboutTab } from "@/components/settings/AboutTab";
import { status, withClient } from "@/test/harness";

const getStatus = vi.fn();
const getVersion = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, getStatus: () => getStatus(), hasBridge: () => true };
});

vi.mock("@tauri-apps/api/app", () => ({ getVersion: () => getVersion() }));

beforeEach(() => {
  getStatus.mockReset();
  getVersion.mockReset();
  getStatus.mockResolvedValue(status({ version: "2.0.0-alpha.0" }));
  getVersion.mockResolvedValue("2.0.0-alpha.1");
});

describe("which build is this", () => {
  it("reports the app's own version and the service's, as separate rows", async () => {
    withClient(<AboutTab />);

    expect(await screen.findByText("CopyPaste 2.0.0-alpha.1")).toBeTruthy();
    expect(await screen.findByText("CopyPaste 2.0.0-alpha.0")).toBeTruthy();
  });

  /** A build with no bridge — the browser, this test — has no bundle to ask.
   *  Saying so beats rendering the service's number under the app's label. */
  it("says the app version is unknown rather than borrowing the service's", async () => {
    getVersion.mockRejectedValue(new Error("no bridge"));
    withClient(<AboutTab />);

    await screen.findByText("CopyPaste 2.0.0-alpha.0");
    expect(screen.getByText("Unknown")).toBeTruthy();
  });
});

describe("the external links", () => {
  it("points at the repository and the release notes", async () => {
    withClient(<AboutTab />);

    const repo = await screen.findByRole("link", { name: "Source code" });
    const notes = screen.getByRole("link", { name: "Release notes" });

    expect(repo.getAttribute("href")).toBe(
      "https://github.com/dmytro-yevs/copypaste",
    );
    expect(notes.getAttribute("href")).toBe(
      "https://github.com/dmytro-yevs/copypaste/releases",
    );
  });

  /** The window is the whole app: a link that navigates it replaces CopyPaste
   *  with a web page and leaves no way back. */
  it("cannot navigate the WebView", async () => {
    withClient(<AboutTab />);

    for (const link of await screen.findAllByRole("link")) {
      expect(link.getAttribute("target")).toBe("_blank");
      expect(link.getAttribute("rel")).toBe("noreferrer");
    }
  });

  /** There is no privacy document to link — the only one in the tree is about
   *  cloud sync, which this build has no account for. A link labelled privacy
   *  that resolves to nothing is worse than no link. */
  it("offers no privacy link", async () => {
    withClient(<AboutTab />);
    await screen.findAllByRole("link");

    expect(screen.queryByRole("link", { name: /privacy/i })).toBeNull();
  });
});
