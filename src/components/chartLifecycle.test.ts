import { describe, expect, it, vi } from "vitest";
import { createStableChartLifecycle, type ChartInstanceLike } from "./chartLifecycle";

describe("stable chart lifecycle", () => {
  it("updates one instance without disposing or reinitializing on data changes", () => {
    const setOption = vi.fn();
    const dispose = vi.fn();
    const chart: ChartInstanceLike = { setOption, resize: vi.fn(), dispose };
    const factory = vi.fn(() => chart);
    const lifecycle = createStableChartLifecycle(factory);
    const target = {} as HTMLElement;
    lifecycle.mount(target);
    lifecycle.update({ series: [] });
    lifecycle.update({ series: [{ id: "system.cpu.usage_pct", type: "line" }] });
    lifecycle.mount(target);
    expect(factory).toHaveBeenCalledTimes(1);
    expect(setOption).toHaveBeenCalledTimes(2);
    expect(dispose).not.toHaveBeenCalled();
    lifecycle.dispose();
    expect(dispose).toHaveBeenCalledTimes(1);
  });

  it("applies a theme palette update through setOption on the existing instance", () => {
    const setOption = vi.fn();
    const dispose = vi.fn();
    const chart: ChartInstanceLike = { setOption, resize: vi.fn(), dispose };
    const factory = vi.fn(() => chart);
    const lifecycle = createStableChartLifecycle(factory);

    lifecycle.mount({} as HTMLElement);
    lifecycle.update({ textStyle: { color: "#111" }, series: [] });
    lifecycle.update({ textStyle: { color: "#eee" }, series: [] });

    expect(factory).toHaveBeenCalledTimes(1);
    expect(setOption).toHaveBeenCalledTimes(2);
    expect(dispose).not.toHaveBeenCalled();
  });
});
