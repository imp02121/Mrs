import { useState } from "react";
import { useBacktestStore } from "@/stores/backtest-store.ts";
import { useRunBacktest } from "@/hooks/useBacktest.ts";
import type { BacktestRunResponse } from "@/types/index.ts";
import InstrumentSelector from "@/components/shared/InstrumentSelector.tsx";

interface ConfigPanelProps {
  onResult: (result: BacktestRunResponse) => void;
}

function Section({
  title,
  defaultOpen = true,
  children,
}: {
  title: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="border-b border-gray-100 last:border-b-0">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between py-3 text-sm font-medium text-gray-700 hover:text-gray-900"
      >
        {title}
        <span className="text-gray-400 text-xs">{open ? "\u25B2" : "\u25BC"}</span>
      </button>
      {open && <div className="pb-4 space-y-3">{children}</div>}
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="block text-xs font-medium text-gray-500 mb-1">{label}</label>
      {children}
    </div>
  );
}

const inputClass =
  "w-full rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-900 font-mono focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500";

const selectClass =
  "w-full rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500";

export default function ConfigPanel({ onResult }: ConfigPanelProps) {
  const store = useBacktestStore();
  const mutation = useRunBacktest();

  const handleRun = () => {
    const request = store.toRequest();
    mutation.mutate(request, {
      onSuccess: (data) => onResult(data),
    });
  };

  return (
    <div className="bg-gray-50 rounded-lg border border-gray-200 p-5">
      <Section title="Instrument & Dates">
        <Field label="Instrument">
          <InstrumentSelector
            value={store.instrument}
            onChange={(v) => store.setField("instrument", v)}
          />
        </Field>
        <div className="grid grid-cols-2 gap-2">
          <Field label="From">
            <input
              type="date"
              value={store.startDate}
              onChange={(e) => store.setField("startDate", e.target.value)}
              className={inputClass}
            />
          </Field>
          <Field label="To">
            <input
              type="date"
              value={store.endDate}
              onChange={(e) => store.setField("endDate", e.target.value)}
              className={inputClass}
            />
          </Field>
        </div>
      </Section>

      <Section title="Stop Loss">
        <Field label="Mode">
          <select
            value={store.slMode}
            onChange={(e) => store.setField("slMode", e.target.value as typeof store.slMode)}
            className={selectClass}
          >
            <option value="SignalBarExtreme">Signal Bar Extreme</option>
            <option value="FixedPoints">Fixed Points</option>
            <option value="Midpoint">Midpoint</option>
          </select>
        </Field>
        {store.slMode === "FixedPoints" && (
          <Field label="Points">
            <input
              type="text"
              value={store.slFixedPoints}
              onChange={(e) => store.setField("slFixedPoints", e.target.value)}
              className={inputClass}
            />
          </Field>
        )}
        {store.slMode === "Midpoint" && (
          <Field label="Midpoint Offset">
            <input
              type="text"
              value={store.slMidpointOffset}
              onChange={(e) => store.setField("slMidpointOffset", e.target.value)}
              className={inputClass}
            />
          </Field>
        )}
        <label className="flex items-center gap-2 text-sm text-gray-700">
          <input
            type="checkbox"
            checked={store.slScaleWithIndex}
            onChange={(e) => store.setField("slScaleWithIndex", e.target.checked)}
            className="rounded border-gray-300"
          />
          Scale with index
        </label>
        {store.slScaleWithIndex && (
          <Field label="Scale Baseline">
            <input
              type="text"
              value={store.slScaleBaseline}
              onChange={(e) => store.setField("slScaleBaseline", e.target.value)}
              className={inputClass}
            />
          </Field>
        )}
      </Section>

      <Section title="Exit Strategy">
        <Field label="Mode">
          <select
            value={store.exitMode}
            onChange={(e) => store.setField("exitMode", e.target.value as typeof store.exitMode)}
            className={selectClass}
          >
            <option value="EndOfDay">End of Day</option>
            <option value="TrailingStop">Trailing Stop</option>
            <option value="FixedTakeProfit">Fixed Take Profit</option>
            <option value="CloseAtTime">Close at Time</option>
            <option value="None">None</option>
          </select>
        </Field>
        {store.exitMode === "EndOfDay" && (
          <Field label="EOD Time">
            <input
              type="time"
              step="1"
              value={store.exitEodTime}
              onChange={(e) => store.setField("exitEodTime", e.target.value)}
              className={inputClass}
            />
          </Field>
        )}
        {store.exitMode === "TrailingStop" && (
          <>
            <Field label="Trail Distance (pts)">
              <input
                type="text"
                value={store.trailingStopDistance}
                onChange={(e) => store.setField("trailingStopDistance", e.target.value)}
                className={inputClass}
              />
            </Field>
            <Field label="Activation (pts)">
              <input
                type="text"
                value={store.trailingStopActivation}
                onChange={(e) => store.setField("trailingStopActivation", e.target.value)}
                className={inputClass}
              />
            </Field>
          </>
        )}
        {store.exitMode === "FixedTakeProfit" && (
          <Field label="Take Profit (pts)">
            <input
              type="text"
              value={store.fixedTpPoints}
              onChange={(e) => store.setField("fixedTpPoints", e.target.value)}
              className={inputClass}
            />
          </Field>
        )}
        {store.exitMode === "CloseAtTime" && (
          <Field label="Close Time">
            <input
              type="time"
              step="1"
              value={store.closeAtTime}
              onChange={(e) => store.setField("closeAtTime", e.target.value)}
              className={inputClass}
            />
          </Field>
        )}
      </Section>

      <Section title="Adding to Winners" defaultOpen={false}>
        <label className="flex items-center gap-2 text-sm text-gray-700">
          <input
            type="checkbox"
            checked={store.addToWinnersEnabled}
            onChange={(e) => store.setField("addToWinnersEnabled", e.target.checked)}
            className="rounded border-gray-300"
          />
          Enable adding to winners
        </label>
        {store.addToWinnersEnabled && (
          <>
            <Field label="Add Every (pts)">
              <input
                type="text"
                value={store.addEveryPoints}
                onChange={(e) => store.setField("addEveryPoints", e.target.value)}
                className={inputClass}
              />
            </Field>
            <Field label="Max Additions">
              <input
                type="number"
                min={1}
                max={10}
                value={store.maxAdditions}
                onChange={(e) => store.setField("maxAdditions", parseInt(e.target.value, 10) || 1)}
                className={inputClass}
              />
            </Field>
            <Field label="Size Multiplier">
              <input
                type="text"
                value={store.addSizeMultiplier}
                onChange={(e) => store.setField("addSizeMultiplier", e.target.value)}
                className={inputClass}
              />
            </Field>
            <label className="flex items-center gap-2 text-sm text-gray-700">
              <input
                type="checkbox"
                checked={store.moveSlOnAdd}
                onChange={(e) => store.setField("moveSlOnAdd", e.target.checked)}
                className="rounded border-gray-300"
              />
              Move SL on add
            </label>
          </>
        )}
      </Section>

      <Section title="Advanced" defaultOpen={false}>
        <Field label="Signal Bar Index">
          <input
            type="number"
            min={1}
            max={10}
            value={store.signalBarIndex}
            onChange={(e) => store.setField("signalBarIndex", parseInt(e.target.value, 10) || 2)}
            className={inputClass}
          />
        </Field>
        <Field label="Entry Offset (pts)">
          <input
            type="text"
            value={store.entryOffsetPoints}
            onChange={(e) => store.setField("entryOffsetPoints", e.target.value)}
            className={inputClass}
          />
        </Field>
        <label className="flex items-center gap-2 text-sm text-gray-700">
          <input
            type="checkbox"
            checked={store.allowBothSides}
            onChange={(e) => store.setField("allowBothSides", e.target.checked)}
            className="rounded border-gray-300"
          />
          Allow both sides
        </label>
        <div className="grid grid-cols-2 gap-2">
          <Field label="Initial Capital">
            <input
              type="text"
              value={store.initialCapital}
              onChange={(e) => store.setField("initialCapital", e.target.value)}
              className={inputClass}
            />
          </Field>
          <Field label="Position Size">
            <input
              type="text"
              value={store.positionSize}
              onChange={(e) => store.setField("positionSize", e.target.value)}
              className={inputClass}
            />
          </Field>
        </div>
        <div className="grid grid-cols-2 gap-2">
          <Field label="Point Value">
            <input
              type="text"
              value={store.pointValue}
              onChange={(e) => store.setField("pointValue", e.target.value)}
              className={inputClass}
            />
          </Field>
          <Field label="Commission">
            <input
              type="text"
              value={store.commissionPerTrade}
              onChange={(e) => store.setField("commissionPerTrade", e.target.value)}
              className={inputClass}
            />
          </Field>
        </div>
        <Field label="Slippage (pts)">
          <input
            type="text"
            value={store.slippagePoints}
            onChange={(e) => store.setField("slippagePoints", e.target.value)}
            className={inputClass}
          />
        </Field>
      </Section>

      <button
        onClick={handleRun}
        disabled={mutation.isPending}
        className="mt-4 w-full rounded-md bg-blue-600 px-4 py-2.5 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {mutation.isPending ? "Running..." : "Run Backtest"}
      </button>

      {mutation.isError && (
        <p className="mt-2 text-sm text-red-600">
          Backtest failed. Please check your configuration and try again.
        </p>
      )}
    </div>
  );
}
