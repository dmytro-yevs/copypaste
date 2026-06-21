import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SliderRow } from "./SliderRow";

// ---------------------------------------------------------------------------
// SliderRow — extracted shared component tests (bdac.3)
// ---------------------------------------------------------------------------

describe("SliderRow", () => {
  it("renders a range input with correct min/max/step/value", () => {
    render(
      <SliderRow
        min={1}
        max={100}
        step={1}
        value={50}
        onChange={() => {}}
        formatValue={(v) => `${v}%`}
      />,
    );
    const input = screen.getByRole("slider") as HTMLInputElement;
    expect(input.min).toBe("1");
    expect(input.max).toBe("100");
    expect(input.step).toBe("1");
    expect(input.value).toBe("50");
  });

  it("displays the formatted value label on the right", () => {
    render(
      <SliderRow
        min={0}
        max={6}
        step={1}
        value={3}
        onChange={() => {}}
        formatValue={(v) => `${v} lines`}
      />,
    );
    expect(screen.getByText("3 lines")).toBeInTheDocument();
  });

  it("calls onChange when the input changes", () => {
    const onChange = vi.fn();
    render(
      <SliderRow
        min={1}
        max={100}
        step={1}
        value={50}
        onChange={onChange}
        formatValue={(v) => String(v)}
      />,
    );
    const input = screen.getByRole("slider");
    fireEvent.change(input, { target: { value: "75" } });
    expect(onChange).toHaveBeenCalledWith(75);
  });

  it("calls onRelease on mouseUp", () => {
    const onRelease = vi.fn();
    render(
      <SliderRow
        min={1}
        max={100}
        step={1}
        value={50}
        onChange={() => {}}
        onRelease={onRelease}
        formatValue={(v) => String(v)}
      />,
    );
    const input = screen.getByRole("slider") as HTMLInputElement;
    // Simulate mouse-up; JSDOM sets currentTarget.value on the event
    fireEvent.mouseUp(input);
    // onRelease is called (value comes from target.value which is "50")
    expect(onRelease).toHaveBeenCalledWith(50);
  });

  it("disables the range input when disabled=true", () => {
    render(
      <SliderRow
        min={1}
        max={100}
        step={1}
        value={50}
        onChange={() => {}}
        formatValue={(v) => String(v)}
        disabled={true}
      />,
    );
    const input = screen.getByRole("slider") as HTMLInputElement;
    expect(input.disabled).toBe(true);
  });

  it("renders a datalist when tickStepCount is provided", () => {
    const { container } = render(
      <SliderRow
        min={0}
        max={4}
        step={1}
        value={2}
        onChange={() => {}}
        formatValue={(v) => String(v)}
        tickStepCount={5}
      />,
    );
    const datalist = container.querySelector("datalist");
    expect(datalist).not.toBeNull();
    // 5 option elements, one per tick
    expect(datalist?.querySelectorAll("option")).toHaveLength(5);
  });

  it("does not render a datalist when tickStepCount is omitted", () => {
    const { container } = render(
      <SliderRow
        min={0}
        max={4}
        step={1}
        value={2}
        onChange={() => {}}
        formatValue={(v) => String(v)}
      />,
    );
    expect(container.querySelector("datalist")).toBeNull();
  });
});
