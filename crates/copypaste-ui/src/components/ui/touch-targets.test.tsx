import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

function expectClass(element: Element, className: string) {
  expect(element.getAttribute("class")).toContain(className);
}

describe("primitive touch targets", () => {
  it("uses the pointer-aware minimum for form controls", () => {
    render(
      <>
        <Input aria-label="Name" />
        <Switch aria-label="Enabled" />
        <Checkbox aria-label="Selected" />
      </>,
    );

    expectClass(screen.getByRole("textbox"), "min-h-[var(--tap-min)]");
    expectClass(screen.getByRole("switch"), "h-[var(--tap-min)]");
    expectClass(screen.getByRole("switch"), "w-12");
    expectClass(screen.getByRole("checkbox"), "size-[var(--tap-min)]");
    expectClass(
      document.querySelector('[data-slot="switch-track"]')!,
      "h-7",
    );
    expectClass(
      document.querySelector('[data-slot="checkbox-control"]')!,
      "size-4",
    );
  });

  it("keeps navigation and dismissal targets pointer-aware", () => {
    render(
      <>
        <Tabs defaultValue="one">
          <TabsList>
            <TabsTrigger value="one">One</TabsTrigger>
          </TabsList>
        </Tabs>
        <ToggleGroup type="single">
          <ToggleGroupItem value="one">One</ToggleGroupItem>
        </ToggleGroup>
        <Dialog open>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Title</DialogTitle>
            </DialogHeader>
          </DialogContent>
        </Dialog>
      </>,
    );

    expectClass(screen.getByRole("tab", { hidden: true }), "min-h-[var(--tap-min)]");
    expectClass(screen.getByRole("tab", { hidden: true }), "min-w-[var(--tap-min)]");
    expectClass(screen.getByRole("radio", { hidden: true }), "min-h-[var(--tap-min)]");
    expectClass(screen.getByRole("radio", { hidden: true }), "min-w-[var(--tap-min)]");
    expectClass(
      document.querySelector('[data-slot="dialog-close"]')!,
      "size-[var(--sz-iconbtn)]",
    );
  });

  it("can divide a segmented control into equal-width items", () => {
    render(
      <ToggleGroup type="single" equalWidth aria-label="Theme">
        <ToggleGroupItem value="system">System</ToggleGroupItem>
        <ToggleGroupItem value="dark">Dark</ToggleGroupItem>
        <ToggleGroupItem value="light">Light</ToggleGroupItem>
      </ToggleGroup>,
    );

    const group = screen.getByRole("radiogroup", { name: "Theme" });
    expect(group.getAttribute("data-equal-width")).toBe("true");
    expectClass(group, "grid");
    for (const item of screen.getAllByRole("radio", { hidden: true })) {
      expectClass(item, "w-full");
      expectClass(item, "min-w-0");
    }
  });

  it("can divide pill tabs into equal-width items", () => {
    render(
      <Tabs defaultValue="overview">
        <TabsList equalWidth aria-label="Views">
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="events">Runtime events</TabsTrigger>
        </TabsList>
      </Tabs>,
    );

    const list = screen.getByRole("tablist", { name: "Views" });
    expect(list.getAttribute("data-equal-width")).toBe("true");
    expectClass(list, "grid");
    expectClass(list, "auto-cols-fr");
  });
});
