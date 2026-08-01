import { afterEach, describe, expect, it, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";

import { AppToaster } from "@/components/shell/AppToaster";

const toaster = vi.fn((_props: unknown) => null);

vi.mock("sonner", () => ({ Toaster: (props: unknown) => toaster(props) }));

afterEach(() => {
  toaster.mockClear();
  document.documentElement.dataset.theme = "dark";
});

describe("AppToaster", () => {
  it("uses the resolved CopyPaste theme instead of Sonner's light default", () => {
    document.documentElement.dataset.theme = "dark";
    render(<AppToaster />);

    expect(toaster).toHaveBeenLastCalledWith(
      expect.objectContaining({ theme: "dark" }),
    );
  });

  it("expands transient feedback into a compact vertical stack", () => {
    render(<AppToaster />);

    expect(toaster).toHaveBeenLastCalledWith(
      expect.objectContaining({ expand: true, gap: 8, visibleToasts: 4 }),
    );
  });

  it("follows a resolved appearance change", async () => {
    document.documentElement.dataset.theme = "dark";
    render(<AppToaster />);

    document.documentElement.dataset.theme = "light";
    await waitFor(() =>
      expect(toaster).toHaveBeenLastCalledWith(
        expect.objectContaining({ theme: "light" }),
      ),
    );
  });

  it("measures the content pane instead of assuming a sidebar width", () => {
    const pane = document.createElement("main");
    pane.dataset.contentPane = "";
    pane.getBoundingClientRect = () =>
      ({ left: 240, width: 960 }) as DOMRect;
    document.body.append(pane);

    render(<AppToaster />);

    expect(document.documentElement.style.getPropertyValue("--content-pane-center")).toBe("720px");
    pane.remove();
  });
});
