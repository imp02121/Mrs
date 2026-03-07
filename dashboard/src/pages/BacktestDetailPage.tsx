import { useParams } from "react-router-dom";
import { useBacktest } from "@/hooks/useBacktest.ts";
import type { BacktestResult } from "@/types/index.ts";
import ResultsPanel from "@/components/backtest/ResultsPanel.tsx";

export default function BacktestDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { data, isLoading, isError } = useBacktest(id);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-gray-400 text-sm">Loading backtest...</div>
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-red-600 text-sm">
          Failed to load backtest. It may not exist.
        </div>
      </div>
    );
  }

  const result = data as { result: BacktestResult };

  return (
    <div>
      <h2 className="text-lg font-semibold text-gray-900 mb-4">
        Backtest Detail
      </h2>
      <ResultsPanel result={result.result} />
    </div>
  );
}
