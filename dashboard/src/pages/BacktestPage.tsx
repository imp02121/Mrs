import { useState } from "react";
import type { BacktestResult, BacktestRunResponse } from "@/types/index.ts";
import ConfigPanel from "@/components/backtest/ConfigPanel.tsx";
import ResultsPanel from "@/components/backtest/ResultsPanel.tsx";

export default function BacktestPage() {
  const [result, setResult] = useState<BacktestResult | null>(null);

  const handleResult = (response: BacktestRunResponse) => {
    setResult(response.result);
  };

  return (
    <div className="flex gap-6">
      <div className="w-[360px] shrink-0">
        <ConfigPanel onResult={handleResult} />
      </div>
      <div className="flex-1 min-w-0">
        {result ? (
          <ResultsPanel result={result} />
        ) : (
          <div className="flex items-center justify-center h-64 text-gray-400 text-sm">
            Configure and run a backtest to see results
          </div>
        )}
      </div>
    </div>
  );
}
