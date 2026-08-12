import { create } from "zustand";

type Page = "today" | "timeline" | "resources" | "apps" | "settings";
export type ThemeMode = "system" | "light" | "dark";

type UiStore = {
  page: Page;
  selectedDate: string;
  showHiddenApps: boolean;
  sidebarCollapsed: boolean;
  themeMode: ThemeMode;
  resolvedTheme: "light" | "dark";
  setPage: (page: Page) => void;
  setSelectedDate: (date: string) => void;
  setShowHiddenApps: (showHiddenApps: boolean) => void;
  toggleSidebar: () => void;
  setThemeMode: (themeMode: ThemeMode) => void;
  setResolvedTheme: (resolvedTheme: "light" | "dark") => void;
};

const now = new Date();
const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
const storedSidebarState = typeof window !== "undefined" && window.localStorage.getItem("resource-timeline-sidebar-collapsed") === "true";
const storedTheme = typeof window !== "undefined" ? window.localStorage.getItem("resource-timeline-theme") : null;
const initialTheme: ThemeMode = storedTheme === "light" || storedTheme === "dark" ? storedTheme : "system";
const initialResolvedTheme = initialTheme === "system"
  ? typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
  : initialTheme;

export const useUiStore = create<UiStore>((set) => ({
  page: "today",
  selectedDate: today,
  showHiddenApps: false,
  sidebarCollapsed: storedSidebarState,
  themeMode: initialTheme,
  resolvedTheme: initialResolvedTheme,
  setPage: (page) => set({ page }),
  setSelectedDate: (selectedDate) => set({ selectedDate }),
  setShowHiddenApps: (showHiddenApps) => set({ showHiddenApps }),
  toggleSidebar: () => set((state) => {
    const sidebarCollapsed = !state.sidebarCollapsed;
    window.localStorage.setItem("resource-timeline-sidebar-collapsed", String(sidebarCollapsed));
    return { sidebarCollapsed };
  }),
  setThemeMode: (themeMode) => {
    window.localStorage.setItem("resource-timeline-theme", themeMode);
    set({ themeMode });
  },
  setResolvedTheme: (resolvedTheme) => set({ resolvedTheme })
}));
