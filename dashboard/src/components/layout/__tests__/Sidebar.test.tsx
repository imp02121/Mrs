import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import Sidebar from "@/components/layout/Sidebar.tsx";

function renderSidebar() {
  return render(
    <MemoryRouter>
      <Sidebar />
    </MemoryRouter>,
  );
}

describe("Sidebar", () => {
  it("should render the app title", () => {
    renderSidebar();
    expect(screen.getByText("School Run")).toBeInTheDocument();
  });

  it("should render the subtitle", () => {
    renderSidebar();
    expect(screen.getByText("Trading Backtester")).toBeInTheDocument();
  });

  it("should render all 5 nav links", () => {
    renderSidebar();
    expect(screen.getByText("Backtest")).toBeInTheDocument();
    expect(screen.getByText("Compare")).toBeInTheDocument();
    expect(screen.getByText("History")).toBeInTheDocument();
    expect(screen.getByText("Signals")).toBeInTheDocument();
    expect(screen.getByText("Data")).toBeInTheDocument();
  });

  it("should have correct link destinations", () => {
    renderSidebar();

    const backtest = screen.getByText("Backtest").closest("a");
    expect(backtest).toHaveAttribute("href", "/");

    const compare = screen.getByText("Compare").closest("a");
    expect(compare).toHaveAttribute("href", "/compare");

    const history = screen.getByText("History").closest("a");
    expect(history).toHaveAttribute("href", "/history");

    const signals = screen.getByText("Signals").closest("a");
    expect(signals).toHaveAttribute("href", "/signals");

    const data = screen.getByText("Data").closest("a");
    expect(data).toHaveAttribute("href", "/data");
  });

  it("should render navigation icons", () => {
    const { container } = renderSidebar();
    const links = container.querySelectorAll("a");
    expect(links).toHaveLength(5);
  });
});
