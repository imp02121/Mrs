import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import MonthlyHeatmap from "@/components/backtest/MonthlyHeatmap.tsx";
import type { DailyPnl } from "@/types/index.ts";

describe("MonthlyHeatmap", () => {
  it("should render the heading", () => {
    render(<MonthlyHeatmap data={[]} />);
    expect(screen.getByText("Monthly PnL Heatmap")).toBeInTheDocument();
  });

  it("should render month headers", () => {
    const data: DailyPnl[] = [
      { date: "2024-01-15", pnl: "100.00", cumulative: "100.00" },
    ];
    render(<MonthlyHeatmap data={data} />);

    const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    for (const m of months) {
      expect(screen.getByText(m)).toBeInTheDocument();
    }
  });

  it("should render correct year rows", () => {
    const data: DailyPnl[] = [
      { date: "2023-06-15", pnl: "50.00", cumulative: "50.00" },
      { date: "2024-03-10", pnl: "-30.00", cumulative: "20.00" },
    ];
    render(<MonthlyHeatmap data={data} />);

    expect(screen.getByText("2023")).toBeInTheDocument();
    expect(screen.getByText("2024")).toBeInTheDocument();
  });

  it("should aggregate daily PnL into monthly buckets", () => {
    const data: DailyPnl[] = [
      { date: "2024-01-05", pnl: "100.00", cumulative: "100.00" },
      { date: "2024-01-10", pnl: "50.00", cumulative: "150.00" },
      { date: "2024-01-20", pnl: "-30.00", cumulative: "120.00" },
    ];
    render(<MonthlyHeatmap data={data} />);

    // 100 + 50 - 30 = 120, formatted as +120
    expect(screen.getByText("+120")).toBeInTheDocument();
  });

  it("should show negative values without plus sign", () => {
    const data: DailyPnl[] = [
      { date: "2024-02-05", pnl: "-200.00", cumulative: "-200.00" },
      { date: "2024-02-10", pnl: "50.00", cumulative: "-150.00" },
    ];
    render(<MonthlyHeatmap data={data} />);

    // -200 + 50 = -150, formatted as -150
    expect(screen.getByText("-150")).toBeInTheDocument();
  });

  it("should show dashes for months with no data", () => {
    const data: DailyPnl[] = [
      { date: "2024-01-15", pnl: "100.00", cumulative: "100.00" },
    ];
    const { container } = render(<MonthlyHeatmap data={data} />);

    // Should have 11 dashes for the 11 months with no data (plus the header row)
    const dashCells = container.querySelectorAll("div.bg-gray-100.text-gray-300");
    expect(dashCells.length).toBe(11);
  });

  it("should handle empty data gracefully", () => {
    const { container } = render(<MonthlyHeatmap data={[]} />);
    expect(screen.getByText("Monthly PnL Heatmap")).toBeInTheDocument();

    // No year rows should be rendered
    const tbody = container.querySelector("tbody");
    expect(tbody).toBeInTheDocument();
    expect(tbody?.querySelectorAll("tr")).toHaveLength(0);
  });

  it("should handle multiple years of data", () => {
    const data: DailyPnl[] = [
      { date: "2023-06-15", pnl: "50.00", cumulative: "50.00" },
      { date: "2024-03-10", pnl: "-30.00", cumulative: "20.00" },
      { date: "2025-11-20", pnl: "75.00", cumulative: "95.00" },
    ];
    render(<MonthlyHeatmap data={data} />);

    expect(screen.getByText("2023")).toBeInTheDocument();
    expect(screen.getByText("2024")).toBeInTheDocument();
    expect(screen.getByText("2025")).toBeInTheDocument();
  });
});
