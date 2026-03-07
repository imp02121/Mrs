import { describe, it, expect, beforeEach } from "vitest";
import { useBacktestStore } from "@/stores/backtest-store.ts";

describe("backtest-store", () => {
  beforeEach(() => {
    // Reset store to defaults before each test
    useBacktestStore.getState().reset();
  });

  it("should have expected default values", () => {
    const state = useBacktestStore.getState();
    expect(state.instrument).toBe("Dax");
    expect(state.startDate).toBe("2024-01-01");
    expect(state.endDate).toBe("2025-12-31");
    expect(state.signalBarIndex).toBe(2);
    expect(state.candleIntervalMinutes).toBe(15);
    expect(state.entryOffsetPoints).toBe("2");
    expect(state.allowBothSides).toBe(true);
    expect(state.slMode).toBe("FixedPoints");
    expect(state.slFixedPoints).toBe("40");
    expect(state.exitMode).toBe("EndOfDay");
    expect(state.addToWinnersEnabled).toBe(false);
    expect(state.maxAdditions).toBe(3);
    expect(state.initialCapital).toBe("100000");
    expect(state.positionSize).toBe("1");
    expect(state.slippagePoints).toBe("0.5");
  });

  it("should update individual fields with setField", () => {
    const { setField } = useBacktestStore.getState();

    setField("instrument", "Ftse");
    expect(useBacktestStore.getState().instrument).toBe("Ftse");

    setField("startDate", "2023-06-01");
    expect(useBacktestStore.getState().startDate).toBe("2023-06-01");

    setField("signalBarIndex", 3);
    expect(useBacktestStore.getState().signalBarIndex).toBe(3);

    setField("allowBothSides", false);
    expect(useBacktestStore.getState().allowBothSides).toBe(false);

    setField("slMode", "Midpoint");
    expect(useBacktestStore.getState().slMode).toBe("Midpoint");

    setField("exitMode", "TrailingStop");
    expect(useBacktestStore.getState().exitMode).toBe("TrailingStop");

    setField("addToWinnersEnabled", true);
    expect(useBacktestStore.getState().addToWinnersEnabled).toBe(true);

    setField("initialCapital", "200000");
    expect(useBacktestStore.getState().initialCapital).toBe("200000");
  });

  it("should reset all fields to defaults", () => {
    const { setField, reset } = useBacktestStore.getState();

    setField("instrument", "Nasdaq");
    setField("startDate", "2020-01-01");
    setField("slMode", "SignalBarExtreme");
    setField("exitMode", "CloseAtTime");
    setField("addToWinnersEnabled", true);
    setField("maxAdditions", 5);
    setField("initialCapital", "500000");

    reset();
    const state = useBacktestStore.getState();

    expect(state.instrument).toBe("Dax");
    expect(state.startDate).toBe("2024-01-01");
    expect(state.slMode).toBe("FixedPoints");
    expect(state.exitMode).toBe("EndOfDay");
    expect(state.addToWinnersEnabled).toBe(false);
    expect(state.maxAdditions).toBe(3);
    expect(state.initialCapital).toBe("100000");
  });

  it("should produce a valid RunBacktestRequest from toRequest()", () => {
    const req = useBacktestStore.getState().toRequest();

    expect(req.instrument).toBe("DAX");
    expect(req.start_date).toBe("2024-01-01");
    expect(req.end_date).toBe("2025-12-31");
    expect(req.config).toBeDefined();
    expect(req.config.instrument).toBe("Dax");
    expect(req.config.signal_bar_index).toBe(2);
    expect(req.config.candle_interval_minutes).toBe(15);
    expect(req.config.entry_offset_points).toBe("2");
    expect(req.config.allow_both_sides).toBe(true);
    expect(req.config.sl_mode).toBe("FixedPoints");
    expect(req.config.sl_fixed_points).toBe("40");
    expect(req.config.exit_mode).toBe("EndOfDay");
    expect(req.config.exit_eod_time).toBe("17:30:00");
    expect(req.config.add_to_winners_enabled).toBe(false);
    expect(req.config.initial_capital).toBe("100000");
    expect(req.config.slippage_points).toBe("0.5");
    expect(req.config.exclude_dates).toEqual([]);
  });

  it("should map instrument names correctly in toRequest()", () => {
    const { setField } = useBacktestStore.getState();

    setField("instrument", "Ftse");
    expect(useBacktestStore.getState().toRequest().instrument).toBe("FTSE");

    setField("instrument", "Nasdaq");
    expect(useBacktestStore.getState().toRequest().instrument).toBe("IXIC");

    setField("instrument", "Dow");
    expect(useBacktestStore.getState().toRequest().instrument).toBe("DJI");

    setField("instrument", "Dax");
    expect(useBacktestStore.getState().toRequest().instrument).toBe("DAX");
  });

  it("should convert empty session strings to null in toRequest()", () => {
    const req = useBacktestStore.getState().toRequest();

    expect(req.config.session_open).toBeNull();
    expect(req.config.session_close).toBeNull();
    expect(req.config.signal_expiry_time).toBeNull();
  });

  it("should preserve non-empty session strings in toRequest()", () => {
    const { setField } = useBacktestStore.getState();

    setField("sessionOpen", "09:00:00");
    setField("sessionClose", "17:30:00");
    setField("signalExpiryTime", "10:00:00");

    const req = useBacktestStore.getState().toRequest();
    expect(req.config.session_open).toBe("09:00:00");
    expect(req.config.session_close).toBe("17:30:00");
    expect(req.config.signal_expiry_time).toBe("10:00:00");
  });
});
