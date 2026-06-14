/**
 * ContentIcon + KindChip shared component tests (CopyPaste-tsb).
 *
 * Verifies:
 *  1. ContentIcon renders the right aria-label / class for each content type.
 *  2. KindChip renders the correct label and color class.
 *  3. density pref round-trips through the store.
 */
import { describe, it, expect, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import { ContentIcon, KindChip } from "./ContentIcon";
import { useUI } from "../store";
import { act } from "react";

// ---------------------------------------------------------------------------
// ContentIcon
// ---------------------------------------------------------------------------

describe("ContentIcon", () => {
  it("renders text icon with accent class for 'text'", () => {
    const { container } = render(<ContentIcon contentType="text" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    // The text icon carries text-ide-accent
    expect(svg!.className.baseVal).toContain("text-ide-accent");
  });

  it("renders text icon with accent class for 'text/plain'", () => {
    const { container } = render(<ContentIcon contentType="text/plain" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg!.className.baseVal).toContain("text-ide-accent");
  });

  it("renders url icon with sky class", () => {
    const { container } = render(<ContentIcon contentType="url" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    // 1hqt: URL uses the sky token (was info)
    expect(svg!.className.baseVal).toContain("text-ide-sky");
  });

  it("renders image icon with sky class for 'image'", () => {
    const { container } = render(<ContentIcon contentType="image" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    // 1hqt: IMAGE uses the sky token (was violet)
    expect(svg!.className.baseVal).toContain("text-ide-sky");
  });

  it("renders image icon with sky class for 'image/png' (MIME prefix)", () => {
    const { container } = render(<ContentIcon contentType="image/png" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg!.className.baseVal).toContain("text-ide-sky");
  });

  it("renders code icon with violet class for 'code'", () => {
    const { container } = render(<ContentIcon contentType="code" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg!.className.baseVal).toContain("text-ide-violet");
  });

  it("renders code icon for 'text/x-python' MIME", () => {
    const { container } = render(<ContentIcon contentType="text/x-python" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg!.className.baseVal).toContain("text-ide-violet");
  });

  it("respects size prop (default 14)", () => {
    const { container } = render(<ContentIcon contentType="text" />);
    const svg = container.querySelector("svg");
    expect(svg!.getAttribute("width")).toBe("14");
    expect(svg!.getAttribute("height")).toBe("14");
  });

  it("respects explicit size prop", () => {
    const { container } = render(<ContentIcon contentType="text" size={20} />);
    const svg = container.querySelector("svg");
    expect(svg!.getAttribute("width")).toBe("20");
    expect(svg!.getAttribute("height")).toBe("20");
  });

  it("renders faint fallback icon for unknown type", () => {
    const { container } = render(<ContentIcon contentType="application/pdf" />);
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    // Falls through to the FileText/faint path — violet for application/*
    // (matches the Popup.tsx ContentChip code/application/* → violet branch)
    expect(svg!.className.baseVal).toContain("text-ide-violet");
  });
});

// ---------------------------------------------------------------------------
// KindChip
// ---------------------------------------------------------------------------

describe("KindChip", () => {
  it("renders TEXT label with accent class", () => {
    const { getByText } = render(<KindChip contentType="text" />);
    const el = getByText("TEXT");
    expect(el.className).toContain("text-ide-accent");
  });

  it("renders URL label with sky class", () => {
    const { getByText } = render(<KindChip contentType="url" />);
    const el = getByText("URL");
    expect(el.className).toContain("text-ide-sky");
  });

  it("renders IMAGE label with sky class", () => {
    const { getByText } = render(<KindChip contentType="image" />);
    const el = getByText("IMAGE");
    expect(el.className).toContain("text-ide-sky");
  });

  it("renders CODE label with violet class", () => {
    const { getByText } = render(<KindChip contentType="code" />);
    const el = getByText("CODE");
    expect(el.className).toContain("text-ide-violet");
  });

  it("prefers explicit kind over contentType-derived label", () => {
    const { getByText } = render(
      <KindChip contentType="text" kind="EMAIL" />
    );
    const el = getByText("EMAIL");
    expect(el.className).toContain("text-ide-success");
  });

  it("renders IMAGE via kind prop", () => {
    const { getByText } = render(
      <KindChip contentType="text" kind="IMAGE" />
    );
    const el = getByText("IMAGE");
    expect(el.className).toContain("text-ide-sky");
  });
});

// ---------------------------------------------------------------------------
// density pref in the Zustand store
// ---------------------------------------------------------------------------

describe("store: density pref", () => {
  beforeEach(() => {
    // Reset the store to defaults before each test.
    act(() => {
      useUI.getState().setPrefs({ density: "comfortable" });
    });
  });

  it("defaults to 'comfortable'", () => {
    expect(useUI.getState().prefs.density).toBe("comfortable");
  });

  it("can be set to 'compact' via setPrefs", () => {
    act(() => {
      useUI.getState().setPrefs({ density: "compact" });
    });
    expect(useUI.getState().prefs.density).toBe("compact");
  });

  it("persists round-trip through setPrefs back to 'comfortable'", () => {
    act(() => {
      useUI.getState().setPrefs({ density: "compact" });
    });
    expect(useUI.getState().prefs.density).toBe("compact");
    act(() => {
      useUI.getState().setPrefs({ density: "comfortable" });
    });
    expect(useUI.getState().prefs.density).toBe("comfortable");
  });
});
