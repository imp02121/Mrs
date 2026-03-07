import { create } from "zustand";
import type {
  ExitMode,
  Instrument,
  RunBacktestRequest,
  StopLossMode,
  StrategyConfig,
} from "@/types/index.ts";

interface BacktestFormState {
  // -- Instrument & dates --
  instrument: Instrument;
  startDate: string;
  endDate: string;

  // -- Signal Detection --
  signalBarIndex: number;
  candleIntervalMinutes: number;
  entryOffsetPoints: string;
  allowBothSides: boolean;

  // -- Stop Loss --
  slMode: StopLossMode;
  slFixedPoints: string;
  slMidpointOffset: string;
  slScaleWithIndex: boolean;
  slScaleBaseline: string;

  // -- Exit Strategy --
  exitMode: ExitMode;
  exitEodTime: string;
  trailingStopDistance: string;
  trailingStopActivation: string;
  fixedTpPoints: string;
  closeAtTime: string;

  // -- Adding to Winners --
  addToWinnersEnabled: boolean;
  addEveryPoints: string;
  maxAdditions: number;
  addSizeMultiplier: string;
  moveSlOnAdd: boolean;
  addSlOffset: string;

  // -- Session Timing --
  sessionOpen: string;
  sessionClose: string;
  signalExpiryTime: string;

  // -- Backtest Scope --
  initialCapital: string;
  positionSize: string;
  pointValue: string;
  commissionPerTrade: string;
  slippagePoints: string;

  // Actions
  setField: <K extends keyof BacktestFormState>(key: K, value: BacktestFormState[K]) => void;
  reset: () => void;
  toRequest: () => RunBacktestRequest;
}

const DEFAULTS = {
  instrument: "Dax" as Instrument,
  startDate: "2024-01-01",
  endDate: "2025-12-31",
  signalBarIndex: 2,
  candleIntervalMinutes: 15,
  entryOffsetPoints: "2",
  allowBothSides: true,
  slMode: "FixedPoints" as StopLossMode,
  slFixedPoints: "40",
  slMidpointOffset: "5",
  slScaleWithIndex: false,
  slScaleBaseline: "12000",
  exitMode: "EndOfDay" as ExitMode,
  exitEodTime: "17:30:00",
  trailingStopDistance: "30",
  trailingStopActivation: "0",
  fixedTpPoints: "100",
  closeAtTime: "15:00:00",
  addToWinnersEnabled: false,
  addEveryPoints: "50",
  maxAdditions: 3,
  addSizeMultiplier: "1",
  moveSlOnAdd: true,
  addSlOffset: "0",
  sessionOpen: "",
  sessionClose: "",
  signalExpiryTime: "",
  initialCapital: "100000",
  positionSize: "1",
  pointValue: "1",
  commissionPerTrade: "0",
  slippagePoints: "0.5",
};

export const useBacktestStore = create<BacktestFormState>((set, get) => ({
  ...DEFAULTS,

  setField: (key, value) => set({ [key]: value }),

  reset: () => set(DEFAULTS),

  toRequest: (): RunBacktestRequest => {
    const s = get();

    const instrumentMap: Record<Instrument, string> = {
      Dax: "DAX",
      Ftse: "FTSE",
      Nasdaq: "IXIC",
      Dow: "DJI",
    };

    const config: StrategyConfig = {
      instrument: s.instrument,
      signal_bar_index: s.signalBarIndex,
      candle_interval_minutes: s.candleIntervalMinutes,
      entry_offset_points: s.entryOffsetPoints,
      allow_both_sides: s.allowBothSides,
      sl_mode: s.slMode,
      sl_fixed_points: s.slFixedPoints,
      sl_midpoint_offset: s.slMidpointOffset,
      sl_scale_with_index: s.slScaleWithIndex,
      sl_scale_baseline: s.slScaleBaseline,
      exit_mode: s.exitMode,
      exit_eod_time: s.exitEodTime,
      trailing_stop_distance: s.trailingStopDistance,
      trailing_stop_activation: s.trailingStopActivation,
      fixed_tp_points: s.fixedTpPoints,
      close_at_time: s.closeAtTime,
      add_to_winners_enabled: s.addToWinnersEnabled,
      add_every_points: s.addEveryPoints,
      max_additions: s.maxAdditions,
      add_size_multiplier: s.addSizeMultiplier,
      move_sl_on_add: s.moveSlOnAdd,
      add_sl_offset: s.addSlOffset,
      session_open: s.sessionOpen || null,
      session_close: s.sessionClose || null,
      signal_expiry_time: s.signalExpiryTime || null,
      date_from: s.startDate,
      date_to: s.endDate,
      initial_capital: s.initialCapital,
      position_size: s.positionSize,
      point_value: s.pointValue,
      commission_per_trade: s.commissionPerTrade,
      slippage_points: s.slippagePoints,
      exclude_dates: [],
    };

    return {
      instrument: instrumentMap[s.instrument],
      start_date: s.startDate,
      end_date: s.endDate,
      config,
    };
  },
}));
