import { useTodaySignals } from "@/hooks/useSignals.ts";
import SignalCard from "@/components/signals/SignalCard.tsx";
import type { SignalRow } from "@/types/index.ts";

const INSTRUMENTS = [
  { id: 1, name: "DAX" },
  { id: 2, name: "FTSE" },
  { id: 3, name: "Nasdaq" },
  { id: 4, name: "Dow" },
];

export default function SignalsPage() {
  const { data: signals, isLoading } = useTodaySignals();

  const signalMap = new Map<number, SignalRow>();
  if (signals) {
    for (const s of signals) {
      signalMap.set(s.instrument_id, s);
    }
  }

  const today = new Date().toLocaleDateString("en-US", {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  });

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-gray-900">{"Today's Signals"}</h2>
        <span className="text-sm text-gray-500">{today}</span>
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center h-40 text-gray-400 text-sm">
          Loading signals...
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-6">
          {INSTRUMENTS.map((inst) => (
            <SignalCard key={inst.id} instrumentName={inst.name} signal={signalMap.get(inst.id)} />
          ))}
        </div>
      )}
    </div>
  );
}
