import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import OptimizePage from "@/pages/OptimizePage.tsx";

// Mock the API endpoints
vi.mock("@/api/endpoints.ts", () => ({
  compareBacktests: vi.fn(),
}));

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <OptimizePage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("OptimizePage", () => {
  it("should render without crashing", () => {
    renderPage();
    expect(screen.getByText("Parameter Optimization")).toBeInTheDocument();
  });

  it("should render sweep parameter inputs", () => {
    renderPage();
    expect(screen.getByText("Sweep Parameter 1 (Rows)")).toBeInTheDocument();
    expect(screen.getByText("Sweep Parameter 2 (Columns)")).toBeInTheDocument();
  });

  it("should render the Run Sweep button", () => {
    renderPage();
    expect(screen.getByRole("button", { name: "Run Sweep" })).toBeInTheDocument();
  });

  it("should show combination count", () => {
    renderPage();
    // Default: sl_fixed_points 20-60 step 10 = 5 values, entry_offset 0-5 step 1 = 6 values = 30
    expect(screen.getByText(/30 combinations/)).toBeInTheDocument();
  });
});
