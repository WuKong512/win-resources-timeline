import { useEffect, useRef, type RefObject } from "react";
import * as echarts from "echarts";

export interface ChartInstanceLike {
  setOption: (option: echarts.EChartsOption, opts?: echarts.SetOptionOpts) => void;
  resize: () => void;
  dispose: () => void;
}

export interface StableChartLifecycle<TTarget = HTMLElement> {
  mount: (target: TTarget) => ChartInstanceLike;
  update: (option: echarts.EChartsOption) => void;
  resize: () => void;
  dispose: () => void;
  get: () => ChartInstanceLike | null;
}

export function createStableChartLifecycle(
  factory: (target: HTMLElement) => ChartInstanceLike = (target) => echarts.init(target)
): StableChartLifecycle {
  let chart: ChartInstanceLike | null = null;
  return {
    mount(target) {
      if (!chart) chart = factory(target);
      return chart;
    },
    update(option) {
      chart?.setOption(option, { replaceMerge: ["series"] });
    },
    resize() {
      chart?.resize();
    },
    dispose() {
      if (!chart) return;
      chart.dispose();
      chart = null;
    },
    get() {
      return chart;
    }
  };
}

export function useStableEcharts(ref: RefObject<HTMLDivElement>): StableChartLifecycle {
  const lifecycleRef = useRef<StableChartLifecycle | null>(null);
  if (!lifecycleRef.current) lifecycleRef.current = createStableChartLifecycle();
  const lifecycle = lifecycleRef.current;

  useEffect(() => {
    const target = ref.current;
    if (!target) return;
    lifecycle.mount(target);
    if (typeof ResizeObserver !== "undefined") {
      const observer = new ResizeObserver(() => lifecycle.resize());
      observer.observe(target);
      return () => {
        observer.disconnect();
        lifecycle.dispose();
      };
    }
    window.addEventListener("resize", lifecycle.resize);
    return () => {
      window.removeEventListener("resize", lifecycle.resize);
      lifecycle.dispose();
    };
  }, [lifecycle, ref]);

  return lifecycle;
}
