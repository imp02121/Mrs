import { useState, useMemo, useCallback } from "react";
import type {
  Instrument,
  RunBacktestRequest,
  StrategyConfig,
  CompareResultItem,
} from "@/types/index.ts";
import { useBacktestStore } from "@/stores/backtest-store.ts";
import { useCompareBacktests } from "@/hooks/useBacktest.ts";
import InstrumentSelector from "@/components/shared/InstrumentSelector.tsx";
import DateRangePicker from "@/components/shared/DateRangePicker.tsx";

type SweepParam =
  | "sl_fixed_points"
  | "entry_offset_points"
  | "trailing_stop_distance"
  | "add_every_points";

const SWEEP_PARAM_OPTIONS: { value: SweepParam; label: string }[] = [
  { value: "sl_fixed_points", label: "SL Fixed Points" },
  { value: "entry_offset_points", label: "Entry Offset Points" },
  { value: "trailing_stop_distance", label: "Trailing Stop Distance" },
  { value: "add_every_points", label: "Add Every Points" },
];

const MAX_COMBOS = 500;

interface SweepRange {
  param: SweepParam;
  min: string;
  max: string;
  step: string;
}

type MetricView = "pnl" | "sharpe";

const inputClass =
  "w-full rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-900 font-mono focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500";

const selectClass =
  "w-full rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500";

function generateRange(min: number, max: number, step: number): number[] {
  if (step <= 0 || min > max) return [];
  const values: number[] = [];
  for (let v = min; v <= max + step * 0.001; v += step) {
    values.push(Math.round(v * 1000) / 1000);
  }
  return values;
}

function SweepParamInput({
  label,
  range,
  onChange,
  otherParam,
}: {
  label: string;
  range: SweepRange;
  onChange: (range: SweepRange) => void;
  otherParam: SweepParam;
}) {
  const availableOptions = SWEEP_PARAM_OPTIONS.filter(
    (o) => o.value === range.param || o.value !== otherParam,
  );

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium text-gray-700">{label}</h3>
      <div>
        <label className="block text-xs font-medium text-gray-500 mb-1">Parameter</label>
        <select
          value={range.param}
          onChange={(e) => onChange({ ...range, param: e.target.value as SweepParam })}
          className={selectClass}
        >
          {availableOptions.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </div>
      <div className="grid grid-cols-3 gap-2">
        <div>
          <label className="block text-xs font-medium text-gray-500 mb-1">Min</label>
          <input
            type="number"
            value={range.min}
            onChange={(e) => onChange({ ...range, min: e.target.value })}
            className={inputClass}
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-gray-500 mb-1">Max</label>
          <input
            type="number"
            value={range.max}
            onChange={(e) => onChange({ ...range, max: e.target.value })}
            className={inputClass}
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-gray-500 mb-1">Step</label>
          <input
            type="number"
            value={range.step}
            onChange={(e) => onChange({ ...range, step: e.target.value })}
            className={inputClass}
          />
        </div>
      </div>
    </div>
  );
}

function getMetricValue(item: CompareResultItem, metric: MetricView): number {
  if (metric === "pnl") {
    return parseFloat(item.result.stats.total_pnl);
  }
  return item.result.stats.sharpe_ratio;
}

function getCellColor(value: number, min: number, max: number): string {
  if (max === min) return "bg-gray-100";
  const ratio = (value - min) / (max - min);
  if (ratio >= 0.8) return "bg-emerald-200";
  if (ratio >= 0.6) return "bg-emerald-100";
  if (ratio >= 0.4) return "bg-yellow-100";
  if (ratio >= 0.2) return "bg-orange-100";
  return "bg-red-200";
}

export default function OptimizePage() {
  const store = useBacktestStore();
  const mutation = useCompareBacktests();

  const [instrument, setInstrument] = useState<Instrument>(store.instrument);
  const [startDate, setStartDate] = useState(store.startDate);
  const [endDate, setEndDate] = useState(store.endDate);

  const [range1, setRange1] = useState<SweepRange>({
    param: "sl_fixed_points",
    min: "20",
    max: "60",
    step: "10",
  });

  const [range2, setRange2] = useState<SweepRange>({
    param: "entry_offset_points",
    min: "0",
    max: "5",
    step: "1",
  });

  const [metricView, setMetricView] = useState<MetricView>("pnl");
  const [results, setResults] = useState<CompareResultItem[] | null>(null);

  const values1 = useMemo(
    () =>
      generateRange(
        parseFloat(range1.min) || 0,
        parseFloat(range1.max) || 0,
        parseFloat(range1.step) || 1,
      ),
    [range1.min, range1.max, range1.step],
  );

  const values2 = useMemo(
    () =>
      generateRange(
        parseFloat(range2.min) || 0,
        parseFloat(range2.max) || 0,
        parseFloat(range2.step) || 1,
      ),
    [range2.min, range2.max, range2.step],
  );

  const totalCombos = values1.length * values2.length;
  const exceedsMax = totalCombos > MAX_COMBOS;

  const buildConfigs = useCallback((): RunBacktestRequest[] => {
    const baseRequest = store.toRequest();
    const configs: RunBacktestRequest[] = [];

    const instrumentMap: Record<Instrument, string> = {
      Dax: "DAX",
      Ftse: "FTSE",
      Nasdaq: "IXIC",
      Dow: "DJI",
    };

    for (const v1 of values1) {
      for (const v2 of values2) {
        const config: StrategyConfig = {
          ...baseRequest.config,
          instrument,
          date_from: startDate,
          date_to: endDate,
          [range1.param]: String(v1),
          [range2.param]: String(v2),
        };
        configs.push({
          instrument: instrumentMap[instrument],
          start_date: startDate,
          end_date: endDate,
          config,
        });
      }
    }
    return configs;
  }, [store, instrument, startDate, endDate, range1.param, range2.param, values1, values2]);

  const handleRun = () => {
    if (exceedsMax) return;
    const configs = buildConfigs();
    if (configs.length === 0) return;

    mutation.mutate(
      { configs },
      {
        onSuccess: (data) => setResults(data),
      },
    );
  };

  // Build result lookup: key = "v1|v2" => CompareResultItem
  const resultMap = useMemo(() => {
    if (!results) return null;
    const map = new Map<string, CompareResultItem>();
    let idx = 0;
    for (const v1 of values1) {
      for (const v2 of values2) {
        if (idx < results.length) {
          map.set(`${v1}|${v2}`, results[idx]);
        }
        idx++;
      }
    }
    return map;
  }, [results, values1, values2]);

  const { minVal, maxVal, bestKey } = useMemo(() => {
    if (!resultMap) return { minVal: 0, maxVal: 0, bestKey: "" };
    let min = Infinity;
    let max = -Infinity;
    let best = "";
    for (const [key, item] of resultMap) {
      const val = getMetricValue(item, metricView);
      if (isFinite(val)) {
        if (val < min) min = val;
        if (val > max) {
          max = val;
          best = key;
        }
      }
    }
    return { minVal: min, maxVal: max, bestKey: best };
  }, [resultMap, metricView]);

  const param1Label =
    SWEEP_PARAM_OPTIONS.find((o) => o.value === range1.param)?.label ?? range1.param;
  const param2Label =
    SWEEP_PARAM_OPTIONS.find((o) => o.value === range2.param)?.label ?? range2.param;

  return (
    <div className="space-y-6">
      <h2 className="text-lg font-semibold text-gray-900">Parameter Optimization</h2>

      <div className="bg-gray-50 rounded-lg border border-gray-200 p-5 space-y-5">
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">Instrument</label>
            <InstrumentSelector value={instrument} onChange={setInstrument} />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">Date Range</label>
            <DateRangePicker
              from={startDate}
              to={endDate}
              onFromChange={setStartDate}
              onToChange={setEndDate}
            />
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <SweepParamInput
            label="Sweep Parameter 1 (Rows)"
            range={range1}
            onChange={setRange1}
            otherParam={range2.param}
          />
          <SweepParamInput
            label="Sweep Parameter 2 (Columns)"
            range={range2}
            onChange={setRange2}
            otherParam={range1.param}
          />
        </div>

        <div className="flex items-center justify-between">
          <div className="text-sm text-gray-500">
            {totalCombos} combination{totalCombos !== 1 ? "s" : ""}
            {exceedsMax && (
              <span className="ml-2 text-red-600 font-medium">
                Exceeds {MAX_COMBOS} limit. Reduce ranges.
              </span>
            )}
          </div>
          <button
            onClick={handleRun}
            disabled={mutation.isPending || exceedsMax || totalCombos === 0}
            className="rounded-md bg-blue-600 px-5 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {mutation.isPending ? "Running Sweep..." : "Run Sweep"}
          </button>
        </div>

        {mutation.isError && (
          <p className="text-sm text-red-600">
            Sweep failed. Please check your configuration and try again.
          </p>
        )}
      </div>

      {resultMap && (
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-gray-900">Heatmap Results</h3>
            <div className="flex gap-2">
              <button
                onClick={() => setMetricView("pnl")}
                className={`px-3 py-1 text-sm rounded-md border transition-colors ${
                  metricView === "pnl"
                    ? "bg-blue-600 text-white border-blue-600"
                    : "bg-white text-gray-700 border-gray-200 hover:bg-gray-50"
                }`}
              >
                PnL
              </button>
              <button
                onClick={() => setMetricView("sharpe")}
                className={`px-3 py-1 text-sm rounded-md border transition-colors ${
                  metricView === "sharpe"
                    ? "bg-blue-600 text-white border-blue-600"
                    : "bg-white text-gray-700 border-gray-200 hover:bg-gray-50"
                }`}
              >
                Sharpe
              </button>
            </div>
          </div>

          <div className="overflow-x-auto">
            <table className="text-sm border-collapse">
              <thead>
                <tr>
                  <th className="px-3 py-2 text-xs font-medium text-gray-500 border border-gray-200 bg-gray-50">
                    {param1Label} \ {param2Label}
                  </th>
                  {values2.map((v2) => (
                    <th
                      key={v2}
                      className="px-3 py-2 text-xs font-medium text-gray-700 border border-gray-200 bg-gray-50 font-mono"
                    >
                      {v2}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {values1.map((v1) => (
                  <tr key={v1}>
                    <td className="px-3 py-2 text-xs font-medium text-gray-700 border border-gray-200 bg-gray-50 font-mono">
                      {v1}
                    </td>
                    {values2.map((v2) => {
                      const key = `${v1}|${v2}`;
                      const item = resultMap.get(key);
                      if (!item) {
                        return (
                          <td
                            key={key}
                            className="px-3 py-2 border border-gray-200 text-center text-gray-400"
                          >
                            --
                          </td>
                        );
                      }
                      const val = getMetricValue(item, metricView);
                      const displayVal = isFinite(val)
                        ? metricView === "pnl"
                          ? val.toFixed(0)
                          : val.toFixed(2)
                        : "--";
                      const isBest = key === bestKey;
                      const colorClass = isFinite(val)
                        ? getCellColor(val, minVal, maxVal)
                        : "bg-gray-100";

                      return (
                        <td
                          key={key}
                          className={`px-3 py-2 border border-gray-200 text-center font-mono tabular-nums text-xs ${colorClass} ${
                            isBest ? "ring-2 ring-blue-600 ring-inset font-bold" : ""
                          }`}
                        >
                          {displayVal}
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
