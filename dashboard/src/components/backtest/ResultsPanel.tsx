import type { BacktestResult } from "@/types/index.ts";
import StatsCards from "./StatsCards.tsx";
import EquityCurve from "./EquityCurve.tsx";
import MonthlyHeatmap from "./MonthlyHeatmap.tsx";
import DrawdownChart from "./DrawdownChart.tsx";
import TradeTable from "./TradeTable.tsx";

interface ResultsPanelProps {
  result: BacktestResult;
  backtestId?: string;
}

function ExportButtons({ backtestId }: { backtestId?: string }) {
  const handleCsvExport = () => {
    if (!backtestId) return;
    const base = import.meta.env.VITE_API_URL ?? "http://localhost:3001/api";
    const url = `${base}/backtest/${backtestId}/export/csv`;
    const token = localStorage.getItem("sr_token");
    const link = document.createElement("a");
    link.href = token ? `${url}?token=${encodeURIComponent(token)}` : url;
    link.download = `backtest-${backtestId}.csv`;
    link.click();
  };

  return (
    <div className="flex gap-2 print:hidden">
      {backtestId && (
        <button
          onClick={handleCsvExport}
          className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-50 transition-colors"
        >
          Export CSV
        </button>
      )}
      <button
        onClick={() => window.print()}
        className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-50 transition-colors"
      >
        Print Report
      </button>
    </div>
  );
}

export default function ResultsPanel({ result, backtestId }: ResultsPanelProps) {
  return (
    <div className="space-y-6">
      <div className="flex items-center justify-end">
        <ExportButtons backtestId={backtestId} />
      </div>
      <StatsCards stats={result.stats} />
      <EquityCurve data={result.equity_curve} />
      <MonthlyHeatmap data={result.daily_pnl} />
      <DrawdownChart data={result.equity_curve} />
      <TradeTable trades={result.trades} />
    </div>
  );
}
