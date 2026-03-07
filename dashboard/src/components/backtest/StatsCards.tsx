import type { BacktestStats } from "@/types/index.ts";

interface StatsCardsProps {
  stats: BacktestStats;
}

interface StatCardProps {
  label: string;
  value: string;
  colorClass?: string;
}

function StatCard({ label, value, colorClass }: StatCardProps) {
  return (
    <div className="bg-gray-50 rounded-lg border border-gray-200 p-4">
      <p className="text-sm text-gray-500">{label}</p>
      <p
        className={`text-2xl font-mono font-semibold mt-1 tabular-nums ${colorClass ?? "text-gray-900"}`}
      >
        {value}
      </p>
    </div>
  );
}

function formatPf(pf: number | string): string {
  if (typeof pf === "string") return pf;
  return pf.toFixed(2);
}

export default function StatsCards({ stats }: StatsCardsProps) {
  const winRateColor = stats.win_rate >= 50 ? "text-emerald-600" : "text-red-600";
  const pfNum =
    typeof stats.profit_factor === "string" ? parseFloat(stats.profit_factor) : stats.profit_factor;
  const pfColor = !isNaN(pfNum) && pfNum >= 1.0 ? "text-emerald-600" : "text-red-600";
  const pnlNum = parseFloat(stats.total_pnl);
  const pnlColor = pnlNum >= 0 ? "text-emerald-600" : "text-red-600";

  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-4">
      <StatCard label="Total Trades" value={String(stats.total_trades)} />
      <StatCard
        label="Win Rate"
        value={`${stats.win_rate.toFixed(1)}%`}
        colorClass={winRateColor}
      />
      <StatCard label="Profit Factor" value={formatPf(stats.profit_factor)} colorClass={pfColor} />
      <StatCard
        label="Sharpe Ratio"
        value={stats.sharpe_ratio.toFixed(2)}
        colorClass={stats.sharpe_ratio >= 0 ? "text-emerald-600" : "text-red-600"}
      />
      <StatCard label="Max Drawdown" value={stats.max_drawdown} colorClass="text-red-600" />
      <StatCard
        label="Net PnL"
        value={pnlNum >= 0 ? `+${stats.total_pnl}` : stats.total_pnl}
        colorClass={pnlColor}
      />
    </div>
  );
}
