import { QueryClientProvider } from "@tanstack/react-query";
import { act, render } from "@testing-library/react";
import { useRef } from "react";
import { describe, expect, it, vi } from "vitest";

import { testClient } from "@/test/harness";
import { useQuickPasteLifecycle } from "./useQuickPasteLifecycle";

vi.mock("@/lib/ipc", () => ({
  hideWindow: vi.fn(),
  setAllowScreenshots: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@/lib/theme", () => ({ applyAppearance: vi.fn() }));
vi.mock("@/store/prefs", () => ({
  readPrefs: () => ({ allowScreenshots: false }),
}));

function LifecycleProbe() {
  const searchRef = useRef<HTMLInputElement>(null);
  useQuickPasteLifecycle({ searchRef, clearLocalState: vi.fn() });
  return null;
}

describe("useQuickPasteLifecycle", () => {
  it("drops held history pages on hide so their pending work cannot repopulate the popup", () => {
    const client = testClient();
    const view = render(
      <QueryClientProvider client={client}>
        <LifecycleProbe />
      </QueryClientProvider>,
    );
    client.setQueryData(["history", "pages", ""], { pages: ["held"] });
    client.setQueryData(["history", "head"], "held");

    act(() => window.__copypasteFreeMemory?.());

    expect(client.getQueryData(["history", "pages", ""])).toBeUndefined();
    expect(client.getQueryData(["history", "head"])).toBeUndefined();
    view.unmount();
  });
});
