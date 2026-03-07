import { describe, it, expect } from "vitest";
import type {
  RunBacktestRequest,
  BacktestStats,
  Trade,
  EquityPoint,
  DailyPnl,
  BacktestRunSummary,
  CompareRequest,
  SignalRow,
  InstrumentRow,
  CandleRow,
  CandleQuery,
  FetchRequest,
  ConfigRow,
  ConfigResponse,
  CreateConfigRequest,
  CreateConfigResponse,
  RequestOtpBody,
  VerifyOtpBody,
  VerifyOtpResponse,
  MeResponse,
  MessageResponse,
  ApiResponse,
  PaginatedResponse,
  ApiErrorResponse,
  Instrument,
  StopLossMode,
  ExitMode,
  Direction,
  PositionStatus,
  StrategyConfig,
} from "@/types/index.ts";

describe("Type interfaces", () => {
  it("should construct a valid BacktestStats object", () => {
    const stats: BacktestStats = {
      total_trades: 100,
      winning_trades: 55,
      losing_trades: 45,
      win_rate: 55.0,
      total_pnl: "1234.56",
      avg_win: "45.00",
      avg_loss: "-30.00",
      largest_win: "200.00",
      largest_loss: "-150.00",
      profit_factor: 1.65,
      max_drawdown: "-500.00",
      max_drawdown_pct: 5.0,
      sharpe_ratio: 1.2,
      sortino_ratio: 1.5,
      calmar_ratio: 2.0,
      max_consecutive_wins: 8,
      max_consecutive_losses: 4,
      avg_trade_duration_minutes: 120,
      long_trades: 50,
      short_trades: 50,
      long_pnl: "700.00",
      short_pnl: "534.56",
    };

    expect(stats.total_trades).toBe(100);
    expect(stats.win_rate).toBe(55.0);
    expect(stats.profit_factor).toBe(1.65);
  });

  it("should allow profit_factor to be a string for edge cases", () => {
    const stats: BacktestStats = {
      total_trades: 10,
      winning_trades: 10,
      losing_trades: 0,
      win_rate: 100.0,
      total_pnl: "500.00",
      avg_win: "50.00",
      avg_loss: "0",
      largest_win: "100.00",
      largest_loss: "0",
      profit_factor: "Infinity",
      max_drawdown: "0",
      max_drawdown_pct: 0,
      sharpe_ratio: 3.0,
      sortino_ratio: 3.0,
      calmar_ratio: 3.0,
      max_consecutive_wins: 10,
      max_consecutive_losses: 0,
      avg_trade_duration_minutes: 60,
      long_trades: 5,
      short_trades: 5,
      long_pnl: "250.00",
      short_pnl: "250.00",
    };

    expect(stats.profit_factor).toBe("Infinity");
  });

  it("should construct a valid Trade object", () => {
    const trade: Trade = {
      instrument: "Dax",
      direction: "Long",
      entry_price: "18000.50",
      entry_time: "2024-01-15T08:15:00Z",
      exit_price: "18050.50",
      exit_time: "2024-01-15T16:30:00Z",
      stop_loss: "17960.50",
      exit_reason: "EndOfDay",
      pnl_points: "50.00",
      pnl_with_adds: "50.00",
      adds: [],
      size: "1",
    };

    expect(trade.instrument).toBe("Dax");
    expect(trade.direction).toBe("Long");
    expect(trade.adds).toHaveLength(0);
  });

  it("should construct a Trade with AddResult entries", () => {
    const trade: Trade = {
      instrument: "Nasdaq",
      direction: "Short",
      entry_price: "17500.00",
      entry_time: "2024-03-01T14:30:00Z",
      exit_price: "17400.00",
      exit_time: "2024-03-01T20:00:00Z",
      stop_loss: "17550.00",
      exit_reason: "StopLoss",
      pnl_points: "100.00",
      pnl_with_adds: "150.00",
      adds: [
        {
          price: "17450.00",
          time: "2024-03-01T15:00:00Z",
          size: "1",
          pnl_points: "50.00",
        },
      ],
      size: "1",
    };

    expect(trade.adds).toHaveLength(1);
    expect(trade.adds[0].price).toBe("17450.00");
  });

  it("should construct valid EquityPoint and DailyPnl objects", () => {
    const ep: EquityPoint = {
      timestamp: "2024-01-15T16:30:00Z",
      equity: "100500.00",
    };
    const dp: DailyPnl = {
      date: "2024-01-15",
      pnl: "500.00",
      cumulative: "500.00",
    };

    expect(ep.equity).toBe("100500.00");
    expect(dp.date).toBe("2024-01-15");
  });

  it("should construct a valid RunBacktestRequest", () => {
    const config: StrategyConfig = {
      instrument: "Dax",
      signal_bar_index: 2,
      candle_interval_minutes: 15,
      entry_offset_points: "2",
      allow_both_sides: true,
      sl_mode: "FixedPoints",
      sl_fixed_points: "40",
      sl_midpoint_offset: "5",
      sl_scale_with_index: false,
      sl_scale_baseline: "12000",
      exit_mode: "EndOfDay",
      exit_eod_time: "17:30:00",
      trailing_stop_distance: "30",
      trailing_stop_activation: "0",
      fixed_tp_points: "100",
      close_at_time: "15:00:00",
      add_to_winners_enabled: false,
      add_every_points: "50",
      max_additions: 3,
      add_size_multiplier: "1",
      move_sl_on_add: true,
      add_sl_offset: "0",
      session_open: null,
      session_close: null,
      signal_expiry_time: null,
      date_from: "2024-01-01",
      date_to: "2025-12-31",
      initial_capital: "100000",
      position_size: "1",
      point_value: "1",
      commission_per_trade: "0",
      slippage_points: "0.5",
      exclude_dates: [],
    };

    const req: RunBacktestRequest = {
      instrument: "DAX",
      start_date: "2024-01-01",
      end_date: "2025-12-31",
      config,
    };

    expect(req.instrument).toBe("DAX");
    expect(req.config.sl_mode).toBe("FixedPoints");
  });

  it("should accept all valid Instrument values", () => {
    const instruments: Instrument[] = ["Dax", "Ftse", "Nasdaq", "Dow"];
    expect(instruments).toHaveLength(4);
  });

  it("should accept all valid StopLossMode values", () => {
    const modes: StopLossMode[] = ["SignalBarExtreme", "FixedPoints", "Midpoint"];
    expect(modes).toHaveLength(3);
  });

  it("should accept all valid ExitMode values", () => {
    const modes: ExitMode[] = [
      "EndOfDay",
      "TrailingStop",
      "FixedTakeProfit",
      "CloseAtTime",
      "None",
    ];
    expect(modes).toHaveLength(5);
  });

  it("should accept all valid Direction values", () => {
    const dirs: Direction[] = ["Long", "Short"];
    expect(dirs).toHaveLength(2);
  });

  it("should accept all valid PositionStatus values", () => {
    const statuses: PositionStatus[] = [
      "Open",
      "StopLoss",
      "TakeProfit",
      "TrailingStop",
      "EndOfDay",
      "TimeClose",
      "Manual",
    ];
    expect(statuses).toHaveLength(7);
  });

  it("should construct valid API wrapper types", () => {
    const resp: ApiResponse<string> = { data: "hello" };
    expect(resp.data).toBe("hello");

    const paged: PaginatedResponse<number> = {
      data: [1, 2, 3],
      pagination: {
        page: 0,
        per_page: 50,
        total_items: 3,
        total_pages: 1,
      },
    };
    expect(paged.data).toHaveLength(3);
    expect(paged.pagination.total_items).toBe(3);

    const err: ApiErrorResponse = {
      error: {
        code: "not_found",
        message: "Resource not found",
        details: null,
      },
    };
    expect(err.error.code).toBe("not_found");
  });

  it("should construct valid SignalRow", () => {
    const signal: SignalRow = {
      id: "abc-123",
      instrument_id: 1,
      signal_date: "2024-01-15",
      signal_bar_high: "18050.00",
      signal_bar_low: "17950.00",
      buy_level: "18052.00",
      sell_level: "17948.00",
      status: "pending",
      fill_details: null,
      created_at: "2024-01-15T08:30:00Z",
    };

    expect(signal.status).toBe("pending");
  });

  it("should construct valid data types", () => {
    const instrument: InstrumentRow = {
      id: 1,
      symbol: "DAX",
      name: "DAX 40",
      open_time_local: "09:00",
      close_time_local: "17:30",
      timezone: "Europe/Berlin",
      tick_size: "0.5",
    };
    expect(instrument.timezone).toBe("Europe/Berlin");

    const candle: CandleRow = {
      instrument_id: 1,
      timestamp: "2024-01-15T08:15:00Z",
      open: "18000.00",
      high: "18050.00",
      low: "17990.00",
      close: "18040.00",
      volume: 1500,
    };
    expect(candle.volume).toBe(1500);

    const query: CandleQuery = {
      instrument: "DAX",
      from: "2024-01-01",
      to: "2024-12-31",
    };
    expect(query.instrument).toBe("DAX");

    const fetchReq: FetchRequest = {
      instrument: "DAX",
      from: "2024-01-01",
      to: "2024-12-31",
    };
    expect(fetchReq.from).toBe("2024-01-01");
  });

  it("should construct valid config types", () => {
    const config: ConfigRow = {
      id: "uuid-1",
      name: "Default DAX",
      params: {},
      created_at: "2024-01-01T00:00:00Z",
    };
    expect(config.name).toBe("Default DAX");

    const resp: ConfigResponse = {
      id: "uuid-1",
      name: "Default DAX",
      params: {},
      created_at: "2024-01-01T00:00:00Z",
    };
    expect(resp.id).toBe("uuid-1");

    const createReq: CreateConfigRequest = {
      name: "New Config",
      params: { key: "value" },
    };
    expect(createReq.name).toBe("New Config");

    const createResp: CreateConfigResponse = { id: "uuid-2" };
    expect(createResp.id).toBe("uuid-2");
  });

  it("should construct valid auth types", () => {
    const otpReq: RequestOtpBody = { email: "test@example.com" };
    expect(otpReq.email).toBe("test@example.com");

    const verifyReq: VerifyOtpBody = {
      email: "test@example.com",
      otp: "123456",
    };
    expect(verifyReq.otp).toBe("123456");

    const verifyResp: VerifyOtpResponse = {
      token: "jwt-token",
      expires_at: "2024-02-01T00:00:00Z",
    };
    expect(verifyResp.token).toBe("jwt-token");

    const me: MeResponse = { email: "test@example.com", role: "admin" };
    expect(me.role).toBe("admin");

    const msg: MessageResponse = { message: "OTP sent" };
    expect(msg.message).toBe("OTP sent");
  });

  it("should construct BacktestRunSummary", () => {
    const summary: BacktestRunSummary = {
      id: "run-1",
      config_id: "config-1",
      instrument_id: 1,
      start_date: "2024-01-01",
      end_date: "2024-12-31",
      total_trades: 200,
      stats: null,
      duration_ms: 1500,
      created_at: "2024-12-31T23:59:59Z",
    };

    expect(summary.total_trades).toBe(200);
    expect(summary.duration_ms).toBe(1500);
  });

  it("should construct CompareRequest and CompareResultItem", () => {
    const config: StrategyConfig = {
      instrument: "Dax",
      signal_bar_index: 2,
      candle_interval_minutes: 15,
      entry_offset_points: "2",
      allow_both_sides: true,
      sl_mode: "FixedPoints",
      sl_fixed_points: "40",
      sl_midpoint_offset: "5",
      sl_scale_with_index: false,
      sl_scale_baseline: "12000",
      exit_mode: "EndOfDay",
      exit_eod_time: "17:30:00",
      trailing_stop_distance: "30",
      trailing_stop_activation: "0",
      fixed_tp_points: "100",
      close_at_time: "15:00:00",
      add_to_winners_enabled: false,
      add_every_points: "50",
      max_additions: 3,
      add_size_multiplier: "1",
      move_sl_on_add: true,
      add_sl_offset: "0",
      session_open: null,
      session_close: null,
      signal_expiry_time: null,
      date_from: "2024-01-01",
      date_to: "2025-12-31",
      initial_capital: "100000",
      position_size: "1",
      point_value: "1",
      commission_per_trade: "0",
      slippage_points: "0.5",
      exclude_dates: [],
    };

    const compareReq: CompareRequest = {
      configs: [{ instrument: "DAX", start_date: "2024-01-01", end_date: "2024-12-31", config }],
    };
    expect(compareReq.configs).toHaveLength(1);
  });
});
