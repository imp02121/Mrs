import type { DailyPnl } from "@/types/index.ts";

interface MonthlyHeatmapProps {
  data: DailyPnl[];
}

const MONTH_LABELS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

interface MonthlyBucket {
  year: number;
  month: number;
  pnl: number;
}

function aggregateMonthly(data: DailyPnl[]): MonthlyBucket[] {
  const map = new Map<string, MonthlyBucket>();
  for (const d of data) {
    const [yearStr, monthStr] = d.date.split("-");
    const year = parseInt(yearStr, 10);
    const month = parseInt(monthStr, 10);
    const key = `${year}-${month}`;
    const existing = map.get(key);
    if (existing) {
      existing.pnl += parseFloat(d.pnl);
    } else {
      map.set(key, { year, month, pnl: parseFloat(d.pnl) });
    }
  }
  return Array.from(map.values());
}

function getCellColor(pnl: number, maxAbs: number): string {
  if (maxAbs === 0) return "bg-gray-100";
  const intensity = Math.min(Math.abs(pnl) / maxAbs, 1);
  if (pnl > 0) {
    if (intensity > 0.66) return "bg-emerald-500 text-white";
    if (intensity > 0.33) return "bg-emerald-300 text-emerald-900";
    return "bg-emerald-100 text-emerald-800";
  }
  if (pnl < 0) {
    if (intensity > 0.66) return "bg-red-500 text-white";
    if (intensity > 0.33) return "bg-red-300 text-red-900";
    return "bg-red-100 text-red-800";
  }
  return "bg-gray-100 text-gray-500";
}

export default function MonthlyHeatmap({ data }: MonthlyHeatmapProps) {
  const buckets = aggregateMonthly(data);
  const maxAbs = Math.max(...buckets.map((b) => Math.abs(b.pnl)), 1);

  const years = [...new Set(buckets.map((b) => b.year))].sort();
  const lookup = new Map(buckets.map((b) => [`${b.year}-${b.month}`, b]));

  return (
    <div className="bg-gray-50 rounded-lg border border-gray-200 p-4">
      <h3 className="text-sm font-medium text-gray-700 mb-3">Monthly PnL Heatmap</h3>
      <div className="overflow-x-auto">
        <table className="text-xs">
          <thead>
            <tr>
              <th className="pr-2 text-left text-gray-500 font-medium">Year</th>
              {MONTH_LABELS.map((m) => (
                <th key={m} className="px-1 text-center text-gray-500 font-medium w-14">
                  {m}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {years.map((year) => (
              <tr key={year}>
                <td className="pr-2 font-mono text-gray-600 py-0.5">{year}</td>
                {Array.from({ length: 12 }, (_, i) => {
                  const bucket = lookup.get(`${year}-${i + 1}`);
                  const pnl = bucket?.pnl ?? 0;
                  const hasData = bucket !== undefined;
                  return (
                    <td key={i} className="px-0.5 py-0.5">
                      <div
                        className={`rounded px-1 py-1 text-center font-mono tabular-nums ${
                          hasData ? getCellColor(pnl, maxAbs) : "bg-gray-100 text-gray-300"
                        }`}
                        title={
                          hasData
                            ? `${MONTH_LABELS[i]} ${year}: ${pnl >= 0 ? "+" : ""}${pnl.toFixed(1)}`
                            : undefined
                        }
                      >
                        {hasData ? (pnl >= 0 ? `+${pnl.toFixed(0)}` : pnl.toFixed(0)) : "-"}
                      </div>
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
