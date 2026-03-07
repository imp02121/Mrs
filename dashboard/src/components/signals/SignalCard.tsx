import type { SignalRow } from "@/types/index.ts";

interface SignalCardProps {
  instrumentName: string;
  signal: SignalRow | undefined;
}

function StatusBadge({ status }: { status: string }) {
  const lower = status.toLowerCase();
  let classes = "bg-gray-100 text-gray-600";
  if (lower === "pending") classes = "bg-amber-100 text-amber-800";
  else if (lower === "filled") classes = "bg-emerald-100 text-emerald-800";
  else if (lower === "expired") classes = "bg-gray-100 text-gray-600";

  return (
    <span
      className={`inline-block rounded-full px-2.5 py-0.5 text-xs font-medium uppercase ${classes}`}
    >
      {status}
    </span>
  );
}

export default function SignalCard({ instrumentName, signal }: SignalCardProps) {
  return (
    <div className="bg-white rounded-lg border border-gray-200 p-6">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-gray-900">{instrumentName}</h3>
        {signal && <StatusBadge status={signal.status} />}
      </div>

      {signal ? (
        <div className="space-y-3 text-sm">
          <div>
            <p className="text-gray-500 text-xs mb-1">Signal Bar</p>
            <p className="text-gray-700">{signal.signal_date}</p>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <p className="text-gray-500 text-xs">High</p>
              <p className="font-mono tabular-nums text-gray-900">{signal.signal_bar_high}</p>
            </div>
            <div>
              <p className="text-gray-500 text-xs">Low</p>
              <p className="font-mono tabular-nums text-gray-900">{signal.signal_bar_low}</p>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <p className="text-gray-500 text-xs">Buy Level</p>
              <p className="font-mono tabular-nums text-emerald-600 font-medium">
                {signal.buy_level}
              </p>
            </div>
            <div>
              <p className="text-gray-500 text-xs">Sell Level</p>
              <p className="font-mono tabular-nums text-red-600 font-medium">{signal.sell_level}</p>
            </div>
          </div>
        </div>
      ) : (
        <p className="text-gray-400 text-sm">Awaiting signal bar</p>
      )}
    </div>
  );
}
