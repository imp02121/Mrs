import { useState, useCallback } from "react";
import {
  ResponsiveContainer,
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  CartesianGrid,
  Legend,
} from "recharts";
import { useCompareBacktests } from "@/hooks/useBacktest.ts";
import { useBacktestStore } from "@/stores/backtest-store.ts";
import type {
  CompareResultItem,
  RunBacktestRequest,
  Instrument,
  StopLossMode,
  ExitMode,
} from "@/types/index.ts";

const COLORS = ["#2563EB", "#DC2626", "#059669", "#D97706"];

const INSTRUMENT_MAP: Record<Instrument, string> = {
  Dax: "DAX",
  Ftse: "FTSE",
  Nasdaq: "IXIC",
  Dow: "DJI",
};

interface ConfigSlot {
  instrument: Instrument;
  slMode: StopLossMode;
  slFixedPoints: string;
  exitMode: ExitMode;
  label: string;
}

function makeRequest(slot: ConfigSlot, store: ReturnType<typeof useBacktestStore.getState>): RunBacktestRequest {
  const config = store.toRequest().config;
  return {
    instrument: INSTRUMENT_MAP[slot.instrument],
    start_date: store.startDate,
    end_date: store.endDate,
    config: {
      ...config,
      instrument: slot.instrument,
      sl_mode: slot.slMode,
      sl_fixed_points: slot.slFixedPoints,
      exit_mode: slot.exitMode,
    },
  };
}

export default function ComparePage() {
  const store = useBacktestStore();
  const mutation = useCompareBacktests();
  const [slots, setSlots] = useState<ConfigSlot[]>([
    { instrument: "Dax", slMode: "FixedPoints", slFixedPoints: "40", exitMode: "EndOfDay", label: "Config A" },
    { instrument: "Dax", slMode: "FixedPoints", slFixedPoints: "60", exitMode: "EndOfDay", label: "Config B" },
  ]);
  const [results, setResults] = useState<CompareResultItem[] | null>(null);
  const [editingIdx, setEditingIdx] = useState<number | null>(null);

  const addSlot = useCallback(() => {
    if (slots.length >= 4) return;
    const labels = ["Config A", "Config B", "Config C", "Config D"];
    setSlots([
      ...slots,
      {
        instrument: "Dax",
        slMode: "FixedPoints",
        slFixedPoints: "40",
        exitMode: "EndOfDay",
        label: labels[slots.length],
      },
    ]);
  }, [slots]);

  const updateSlot = useCallback(
    (idx: number, partial: Partial<ConfigSlot>) => {
      setSlots((prev) => prev.map((s, i) => (i === idx ? { ...s, ...partial } : s)));
    },
    [],
  );

  const removeSlot = useCallback(
    (idx: number) => {
      if (slots.length <= 2) return;
      setSlots((prev) => prev.filter((_, i) => i !== idx));
    },
    [slots.length],
  );

  const handleRun = useCallback(() => {
    const storeState = useBacktestStore.getState();
    const configs = slots.map((slot) => makeRequest(slot, storeState));
    mutation.mutate(
      { configs },
      { onSuccess: (data) => setResults(data) },
    );
  }, [slots, mutation]);

  const equityData = results
    ? (() => {
        const dateMap = new Map<string, Record<string, number>>();
        results.forEach((r, idx) => {
          for (const pt of r.result.equity_curve) {
            const date = pt.timestamp.slice(0, 10);
            const existing = dateMap.get(date) ?? {};
            existing[`equity_${idx}`] = parseFloat(pt.equity);
            dateMap.set(date, existing);
          }
        });
        return Array.from(dateMap.entries())
          .sort(([a], [b]) => a.localeCompare(b))
          .map(([date, values]) => ({ date, ...values }));
      })()
    : null;

  return (
    <div>
      <h2 className="text-lg font-semibold text-gray-900 mb-4">
        Compare Backtests
      </h2>

      <div className="flex gap-4 mb-4 flex-wrap">
        {slots.map((slot, idx) => (
          <div
            key={idx}
            className="bg-gray-50 rounded-lg border border-gray-200 p-4 w-52"
          >
            <div className="flex items-center justify-between mb-2">
              <span className="text-sm font-medium text-gray-700">
                {slot.label}
              </span>
              {slots.length > 2 && (
                <button
                  onClick={() => removeSlot(idx)}
                  className="text-gray-400 hover:text-red-600 text-xs"
                >
                  Remove
                </button>
              )}
            </div>
            {editingIdx === idx ? (
              <div className="space-y-2">
                <select
                  value={slot.instrument}
                  onChange={(e) =>
                    updateSlot(idx, { instrument: e.target.value as Instrument })
                  }
                  className="w-full text-xs rounded border border-gray-200 px-2 py-1"
                >
                  <option value="Dax">DAX</option>
                  <option value="Ftse">FTSE</option>
                  <option value="Nasdaq">Nasdaq</option>
                  <option value="Dow">Dow</option>
                </select>
                <select
                  value={slot.slMode}
                  onChange={(e) =>
                    updateSlot(idx, { slMode: e.target.value as StopLossMode })
                  }
                  className="w-full text-xs rounded border border-gray-200 px-2 py-1"
                >
                  <option value="SignalBarExtreme">Signal Bar Extreme</option>
                  <option value="FixedPoints">Fixed Points</option>
                  <option value="Midpoint">Midpoint</option>
                </select>
                {slot.slMode === "FixedPoints" && (
                  <input
                    type="text"
                    value={slot.slFixedPoints}
                    onChange={(e) =>
                      updateSlot(idx, { slFixedPoints: e.target.value })
                    }
                    placeholder="SL Points"
                    className="w-full text-xs font-mono rounded border border-gray-200 px-2 py-1"
                  />
                )}
                <select
                  value={slot.exitMode}
                  onChange={(e) =>
                    updateSlot(idx, { exitMode: e.target.value as ExitMode })
                  }
                  className="w-full text-xs rounded border border-gray-200 px-2 py-1"
                >
                  <option value="EndOfDay">End of Day</option>
                  <option value="TrailingStop">Trailing Stop</option>
                  <option value="FixedTakeProfit">Fixed TP</option>
                  <option value="CloseAtTime">Close at Time</option>
                  <option value="None">None</option>
                </select>
                <button
                  onClick={() => setEditingIdx(null)}
                  className="w-full text-xs text-blue-600 hover:text-blue-700 font-medium"
                >
                  Done
                </button>
              </div>
            ) : (
              <>
                <p className="text-xs text-gray-600">
                  {INSTRUMENT_MAP[slot.instrument]}
                </p>
                <p className="text-xs text-gray-600">SL: {slot.slMode}</p>
                {slot.slMode === "FixedPoints" && (
                  <p className="text-xs text-gray-600 font-mono">
                    {slot.slFixedPoints} pts
                  </p>
                )}
                <p className="text-xs text-gray-600">{slot.exitMode}</p>
                <button
                  onClick={() => setEditingIdx(idx)}
                  className="mt-2 text-xs text-blue-600 hover:text-blue-700 font-medium"
                >
                  Edit
                </button>
              </>
            )}
          </div>
        ))}
        {slots.length < 4 && (
          <button
            onClick={addSlot}
            className="flex items-center justify-center w-52 rounded-lg border border-dashed border-gray-300 text-gray-400 hover:text-gray-600 hover:border-gray-400 text-sm"
          >
            + Add Config
          </button>
        )}
      </div>

      <button
        onClick={handleRun}
        disabled={mutation.isPending}
        className="rounded-md bg-blue-600 px-6 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors mb-6"
      >
        {mutation.isPending ? "Running..." : "Run Comparison"}
      </button>

      {mutation.isError && (
        <p className="text-sm text-red-600 mb-4">
          Comparison failed. Check configurations and try again.
        </p>
      )}

      {results && (
        <div className="space-y-6">
          <div className="bg-gray-50 rounded-lg border border-gray-200 p-4 overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-200">
                  <th className="text-left py-2 px-3 text-xs font-medium text-gray-500">
                    Metric
                  </th>
                  {results.map((r, idx) => (
                    <th
                      key={idx}
                      className="text-right py-2 px-3 text-xs font-medium"
                      style={{ color: COLORS[idx] }}
                    >
                      {slots[idx]?.label ?? `Config ${idx + 1}`}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="font-mono tabular-nums text-gray-700">
                {[
                  { label: "Trades", get: (r: CompareResultItem) => String(r.result.stats.total_trades) },
                  { label: "Win Rate", get: (r: CompareResultItem) => `${r.result.stats.win_rate.toFixed(1)}%` },
                  {
                    label: "Profit Factor",
                    get: (r: CompareResultItem) =>
                      typeof r.result.stats.profit_factor === "string"
                        ? r.result.stats.profit_factor
                        : r.result.stats.profit_factor.toFixed(2),
                  },
                  { label: "Sharpe", get: (r: CompareResultItem) => r.result.stats.sharpe_ratio.toFixed(2) },
                  { label: "Max DD", get: (r: CompareResultItem) => r.result.stats.max_drawdown },
                  { label: "Net PnL", get: (r: CompareResultItem) => r.result.stats.total_pnl },
                ].map((row) => (
                  <tr key={row.label} className="border-b border-gray-100">
                    <td className="py-1.5 px-3 font-sans text-gray-600">
                      {row.label}
                    </td>
                    {results.map((r, idx) => (
                      <td key={idx} className="py-1.5 px-3 text-right">
                        {row.get(r)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {equityData && (
            <div className="bg-gray-50 rounded-lg border border-gray-200 p-4">
              <h3 className="text-sm font-medium text-gray-700 mb-3">
                Overlaid Equity Curves
              </h3>
              <ResponsiveContainer width="100%" height={300}>
                <LineChart data={equityData}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#E5E7EB" />
                  <XAxis
                    dataKey="date"
                    tick={{ fontSize: 11, fill: "#6B7280" }}
                    tickLine={false}
                    axisLine={{ stroke: "#E5E7EB" }}
                  />
                  <YAxis
                    tick={{ fontSize: 11, fill: "#6B7280" }}
                    tickLine={false}
                    axisLine={{ stroke: "#E5E7EB" }}
                    width={70}
                  />
                  <Tooltip
                    contentStyle={{
                      fontSize: 12,
                      borderRadius: 6,
                      border: "1px solid #E5E7EB",
                    }}
                  />
                  <Legend />
                  {results.map((_, idx) => (
                    <Line
                      key={idx}
                      type="monotone"
                      dataKey={`equity_${idx}`}
                      name={slots[idx]?.label ?? `Config ${idx + 1}`}
                      stroke={COLORS[idx]}
                      strokeWidth={2}
                      dot={false}
                    />
                  ))}
                </LineChart>
              </ResponsiveContainer>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
