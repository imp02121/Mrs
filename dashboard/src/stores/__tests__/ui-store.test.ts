import { describe, it, expect, beforeEach } from "vitest";
import { useUiStore } from "@/stores/ui-store.ts";

describe("ui-store", () => {
  beforeEach(() => {
    // Reset to default state
    useUiStore.setState({ sidebarCollapsed: false });
  });

  it("should default sidebarCollapsed to false", () => {
    expect(useUiStore.getState().sidebarCollapsed).toBe(false);
  });

  it("should toggle sidebar from false to true", () => {
    useUiStore.getState().toggleSidebar();
    expect(useUiStore.getState().sidebarCollapsed).toBe(true);
  });

  it("should toggle sidebar back to false", () => {
    useUiStore.getState().toggleSidebar();
    expect(useUiStore.getState().sidebarCollapsed).toBe(true);

    useUiStore.getState().toggleSidebar();
    expect(useUiStore.getState().sidebarCollapsed).toBe(false);
  });

  it("should handle multiple toggles correctly", () => {
    const { toggleSidebar } = useUiStore.getState();

    for (let i = 0; i < 10; i++) {
      toggleSidebar();
    }
    // 10 toggles: should be back to false
    expect(useUiStore.getState().sidebarCollapsed).toBe(false);

    toggleSidebar();
    // 11 toggles: should be true
    expect(useUiStore.getState().sidebarCollapsed).toBe(true);
  });
});
