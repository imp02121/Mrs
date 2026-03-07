import type { Instrument } from "@/types/index.ts";

interface InstrumentSelectorProps {
  value: Instrument;
  onChange: (instrument: Instrument) => void;
}

const INSTRUMENTS: { value: Instrument; label: string }[] = [
  { value: "Dax", label: "DAX 40" },
  { value: "Ftse", label: "FTSE 100" },
  { value: "Nasdaq", label: "Nasdaq" },
  { value: "Dow", label: "Dow 30" },
];

export default function InstrumentSelector({
  value,
  onChange,
}: InstrumentSelectorProps) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as Instrument)}
      className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
    >
      {INSTRUMENTS.map((inst) => (
        <option key={inst.value} value={inst.value}>
          {inst.label}
        </option>
      ))}
    </select>
  );
}
