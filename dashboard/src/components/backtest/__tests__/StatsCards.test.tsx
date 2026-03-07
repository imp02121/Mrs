import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import StatsCards from "@/components/backtest/StatsCards.tsx";
import type { BacktestStats } from "@/types/index.ts";

function makeStats(overrides: Partial<BacktestStats> = {}): BacktestStats {
  return {
    total_trades: 100,
    winning_trades: 55,
    losing_trades: 45,
    win_rate: 55.0,
    total_pnl: "1234.56",
    avg_win: "45.00",
    avg_loss: "-30.00",
    largest_win: "200.00",
    largest_loss: "-150.00",
    profit_factor: 1.65,
    max_drawdown: "-500.00",
    max_drawdown_pct: 5.0,
    sharpe_ratio: 1.2,
    sortino_ratio: 1.5,
    calmar_ratio: 2.0,
    max_consecutive_wins: 8,
    max_consecutive_losses: 4,
    avg_trade_duration_minutes: 120,
    long_trades: 50,
    short_trades: 50,
    long_pnl: "700.00",
    short_pnl: "534.56",
    ...overrides,
  };
}

describe("StatsCards", () => {
  it("should render all 6 stat cards", () => {
    render(<StatsCards stats={makeStats()} />);

    expect(screen.getByText("Total Trades")).toBeInTheDocument();
    expect(screen.getByText("Win Rate")).toBeInTheDocument();
    expect(screen.getByText("Profit Factor")).toBeInTheDocument();
    expect(screen.getByText("Sharpe Ratio")).toBeInTheDocument();
    expect(screen.getByText("Max Drawdown")).toBeInTheDocument();
    expect(screen.getByText("Net PnL")).toBeInTheDocument();
  });

  it("should display the correct values", () => {
    render(<StatsCards stats={makeStats()} />);

    expect(screen.getByText("100")).toBeInTheDocument();
    expect(screen.getByText("55.0%")).toBeInTheDocument();
    expect(screen.getByText("1.65")).toBeInTheDocument();
    expect(screen.getByText("1.20")).toBeInTheDocument();
    expect(screen.getByText("-500.00")).toBeInTheDocument();
    expect(screen.getByText("+1234.56")).toBeInTheDocument();
  });

  it("should show emerald/green styling for positive win rate (>=50)", () => {
    render(<StatsCards stats={makeStats({ win_rate: 60.0 })} />);
    const winRateValue = screen.getByText("60.0%");
    expect(winRateValue.className).toContain("text-emerald-600");
  });

  it("should show red styling for low win rate (<50)", () => {
    render(<StatsCards stats={makeStats({ win_rate: 40.0 })} />);
    const winRateValue = screen.getByText("40.0%");
    expect(winRateValue.className).toContain("text-red-600");
  });

  it("should show emerald styling for profit factor >= 1.0", () => {
    render(<StatsCards stats={makeStats({ profit_factor: 2.0 })} />);
    const pfValue = screen.getByText("2.00");
    expect(pfValue.className).toContain("text-emerald-600");
  });

  it("should show red styling for profit factor < 1.0", () => {
    render(<StatsCards stats={makeStats({ profit_factor: 0.8 })} />);
    const pfValue = screen.getByText("0.80");
    expect(pfValue.className).toContain("text-red-600");
  });

  it("should show emerald styling for positive net PnL", () => {
    render(<StatsCards stats={makeStats({ total_pnl: "500.00" })} />);
    const pnlValue = screen.getByText("+500.00");
    expect(pnlValue.className).toContain("text-emerald-600");
  });

  it("should show red styling for negative net PnL", () => {
    render(<StatsCards stats={makeStats({ total_pnl: "-300.00", max_drawdown: "-200.00" })} />);
    const pnlValue = screen.getByText("-300.00");
    expect(pnlValue.className).toContain("text-red-600");
  });

  it("should show emerald styling for positive Sharpe ratio", () => {
    render(<StatsCards stats={makeStats({ sharpe_ratio: 1.5 })} />);
    const sharpeValue = screen.getByText("1.50");
    expect(sharpeValue.className).toContain("text-emerald-600");
  });

  it("should show red styling for negative Sharpe ratio", () => {
    render(<StatsCards stats={makeStats({ sharpe_ratio: -0.5 })} />);
    const sharpeValue = screen.getByText("-0.50");
    expect(sharpeValue.className).toContain("text-red-600");
  });

  it("should handle string profit factor", () => {
    render(<StatsCards stats={makeStats({ profit_factor: "Infinity" })} />);
    expect(screen.getByText("Infinity")).toBeInTheDocument();
  });

  it("should format numbers correctly", () => {
    render(
      <StatsCards
        stats={makeStats({
          win_rate: 66.666,
          sharpe_ratio: 1.234,
          profit_factor: 3.141,
        })}
      />,
    );

    expect(screen.getByText("66.7%")).toBeInTheDocument();
    expect(screen.getByText("1.23")).toBeInTheDocument();
    expect(screen.getByText("3.14")).toBeInTheDocument();
  });
});
