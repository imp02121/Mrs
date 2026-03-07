import {
  ResponsiveContainer,
  AreaChart,
  Area,
  XAxis,
  YAxis,
  Tooltip,
  CartesianGrid,
} from "recharts";
import type { EquityPoint } from "@/types/index.ts";

interface DrawdownChartProps {
  data: EquityPoint[];
}

export default function DrawdownChart({ data }: DrawdownChartProps) {
  let peak = -Infinity;
  const chartData = data.map((pt) => {
    const equity = parseFloat(pt.equity);
    if (equity > peak) peak = equity;
    const drawdown = peak > 0 ? ((equity - peak) / peak) * 100 : 0;
    return {
      date: pt.timestamp.slice(0, 10),
      drawdown: parseFloat(drawdown.toFixed(2)),
    };
  });

  return (
    <div className="bg-gray-50 rounded-lg border border-gray-200 p-4">
      <h3 className="text-sm font-medium text-gray-700 mb-3">Drawdown</h3>
      <ResponsiveContainer width="100%" height={200}>
        <AreaChart data={chartData}>
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
            width={50}
            tickFormatter={(v: number) => `${v}%`}
          />
          <Tooltip
            formatter={(value: number) => [`${value}%`, "Drawdown"]}
            contentStyle={{
              fontSize: 12,
              borderRadius: 6,
              border: "1px solid #E5E7EB",
            }}
          />
          <Area
            type="monotone"
            dataKey="drawdown"
            stroke="#DC2626"
            fill="#DC2626"
            fillOpacity={0.15}
            strokeWidth={1.5}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
