import { useState, useRef, useEffect } from "react";
import { useInstruments, useCandles } from "@/hooks/useData.ts";
import type { CandleQuery, CandleRow } from "@/types/index.ts";

export default function DataPage() {
  const { data: instruments, isLoading: loadingInst } = useInstruments();
  const [candleQuery, setCandleQuery] = useState<CandleQuery | null>(null);
  const [formInstrument, setFormInstrument] = useState("DAX");
  const [formFrom, setFormFrom] = useState("2024-01-01");
  const [formTo, setFormTo] = useState("2024-01-31");
  const { data: candles, isLoading: loadingCandles } = useCandles(candleQuery);
  const chartContainerRef = useRef<HTMLDivElement>(null);

  const handleLoad = () => {
    setCandleQuery({
      instrument: formInstrument,
      from: formFrom,
      to: formTo,
    });
  };

  useEffect(() => {
    if (!candles || candles.length === 0 || !chartContainerRef.current) return;

    let chart: ReturnType<typeof import("lightweight-charts").createChart> | null = null;

    void (async () => {
      const { createChart, CandlestickSeries } = await import("lightweight-charts");
      if (!chartContainerRef.current) return;

      chartContainerRef.current.innerHTML = "";
      chart = createChart(chartContainerRef.current, {
        width: chartContainerRef.current.clientWidth,
        height: 400,
        layout: {
          background: { color: "#FFFFFF" },
          textColor: "#6B7280",
          fontSize: 11,
        },
        grid: {
          vertLines: { color: "#F3F4F6" },
          horzLines: { color: "#F3F4F6" },
        },
      });

      const series = chart.addSeries(CandlestickSeries, {
        upColor: "#059669",
        downColor: "#DC2626",
        borderUpColor: "#059669",
        borderDownColor: "#DC2626",
        wickUpColor: "#059669",
        wickDownColor: "#DC2626",
      });

      const chartData = candles.map((c: CandleRow) => ({
        time: c.timestamp.slice(0, 10) as string,
        open: parseFloat(c.open),
        high: parseFloat(c.high),
        low: parseFloat(c.low),
        close: parseFloat(c.close),
      }));

      series.setData(chartData);
      chart.timeScale().fitContent();
    })();

    return () => {
      if (chart) chart.remove();
    };
  }, [candles]);

  return (
    <div>
      <h2 className="text-lg font-semibold text-gray-900 mb-4">Data</h2>

      <div className="bg-gray-50 rounded-lg border border-gray-200 p-4 mb-6">
        <h3 className="text-sm font-medium text-gray-700 mb-3">Instruments</h3>
        {loadingInst ? (
          <p className="text-sm text-gray-400">Loading...</p>
        ) : instruments && instruments.length > 0 ? (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-gray-200">
                <th className="text-left py-2 px-3 text-xs font-medium text-gray-500">Symbol</th>
                <th className="text-left py-2 px-3 text-xs font-medium text-gray-500">Name</th>
                <th className="text-left py-2 px-3 text-xs font-medium text-gray-500">Open</th>
                <th className="text-left py-2 px-3 text-xs font-medium text-gray-500">Close</th>
                <th className="text-left py-2 px-3 text-xs font-medium text-gray-500">Timezone</th>
              </tr>
            </thead>
            <tbody>
              {instruments.map((inst) => (
                <tr key={inst.id} className="border-b border-gray-100">
                  <td className="py-1.5 px-3 font-mono font-medium text-gray-900">{inst.symbol}</td>
                  <td className="py-1.5 px-3 text-gray-700">{inst.name}</td>
                  <td className="py-1.5 px-3 font-mono text-gray-600">{inst.open_time_local}</td>
                  <td className="py-1.5 px-3 font-mono text-gray-600">{inst.close_time_local}</td>
                  <td className="py-1.5 px-3 text-gray-500 text-xs">{inst.timezone}</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p className="text-sm text-gray-400">No instruments found</p>
        )}
      </div>

      <div className="bg-gray-50 rounded-lg border border-gray-200 p-4">
        <h3 className="text-sm font-medium text-gray-700 mb-3">Candle Explorer</h3>
        <div className="flex items-end gap-3 mb-4">
          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">Instrument</label>
            <select
              value={formInstrument}
              onChange={(e) => setFormInstrument(e.target.value)}
              className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
            >
              <option value="DAX">DAX</option>
              <option value="FTSE">FTSE</option>
              <option value="IXIC">Nasdaq</option>
              <option value="DJI">Dow</option>
            </select>
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">From</label>
            <input
              type="date"
              value={formFrom}
              onChange={(e) => setFormFrom(e.target.value)}
              className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
          </div>
          <div>
            <label className="block text-xs font-medium text-gray-500 mb-1">To</label>
            <input
              type="date"
              value={formTo}
              onChange={(e) => setFormTo(e.target.value)}
              className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
          </div>
          <button
            onClick={handleLoad}
            className="rounded-md bg-blue-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
          >
            Load Candles
          </button>
        </div>

        {loadingCandles && <p className="text-sm text-gray-400">Loading candles...</p>}

        {candles && candles.length > 0 && (
          <>
            <p className="text-sm text-gray-500 mb-3">
              Showing {candles.length} candles for {formInstrument}
            </p>

            <div ref={chartContainerRef} className="mb-4 rounded border border-gray-200" />

            <div className="max-h-64 overflow-auto rounded border border-gray-200">
              <table className="w-full text-xs">
                <thead className="sticky top-0 bg-white">
                  <tr className="border-b border-gray-200">
                    <th className="text-left py-1.5 px-2 font-medium text-gray-500">Timestamp</th>
                    <th className="text-right py-1.5 px-2 font-medium text-gray-500">Open</th>
                    <th className="text-right py-1.5 px-2 font-medium text-gray-500">High</th>
                    <th className="text-right py-1.5 px-2 font-medium text-gray-500">Low</th>
                    <th className="text-right py-1.5 px-2 font-medium text-gray-500">Close</th>
                    <th className="text-right py-1.5 px-2 font-medium text-gray-500">Volume</th>
                  </tr>
                </thead>
                <tbody>
                  {candles.map((c, i) => (
                    <tr key={i} className="border-b border-gray-50">
                      <td className="py-1 px-2 text-gray-600">
                        {c.timestamp.replace("T", " ").slice(0, 19)}
                      </td>
                      <td className="py-1 px-2 text-right font-mono tabular-nums">{c.open}</td>
                      <td className="py-1 px-2 text-right font-mono tabular-nums">{c.high}</td>
                      <td className="py-1 px-2 text-right font-mono tabular-nums">{c.low}</td>
                      <td className="py-1 px-2 text-right font-mono tabular-nums">{c.close}</td>
                      <td className="py-1 px-2 text-right font-mono tabular-nums text-gray-500">
                        {c.volume}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </>
        )}

        {candles && candles.length === 0 && (
          <p className="text-sm text-gray-400">No candles found for the selected range</p>
        )}
      </div>
    </div>
  );
}
