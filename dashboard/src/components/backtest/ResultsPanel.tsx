import type { BacktestResult } from "@/types/index.ts";
import StatsCards from "./StatsCards.tsx";
import EquityCurve from "./EquityCurve.tsx";
import MonthlyHeatmap from "./MonthlyHeatmap.tsx";
import DrawdownChart from "./DrawdownChart.tsx";
import TradeTable from "./TradeTable.tsx";

interface ResultsPanelProps {
  result: BacktestResult;
}

export default function ResultsPanel({ result }: ResultsPanelProps) {
  return (
    <div className="space-y-6">
      <StatsCards stats={result.stats} />
      <EquityCurve data={result.equity_curve} />
      <MonthlyHeatmap data={result.daily_pnl} />
      <DrawdownChart data={result.equity_curve} />
      <TradeTable trades={result.trades} />
    </div>
  );
}
