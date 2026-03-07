export type {
  ApiResponse,
  PaginatedResponse,
  Pagination,
  ApiErrorResponse,
  ApiErrorDetail,
} from "./api.ts";

export type {
  StopLossMode,
  ExitMode,
  Direction,
  PositionStatus,
  Instrument,
  StrategyConfig,
} from "./strategy.ts";

export type {
  RunBacktestRequest,
  BacktestRunResponse,
  BacktestResult,
  BacktestStats,
  Trade,
  AddResult,
  EquityPoint,
  DailyPnl,
  BacktestRunSummary,
  CompareResultItem,
  CompareRequest,
} from "./backtest.ts";

export type { SignalRow } from "./signal.ts";

export type {
  InstrumentRow,
  CandleRow,
  CandleQuery,
  FetchRequest,
} from "./data.ts";

export type {
  ConfigRow,
  ConfigResponse,
  CreateConfigRequest,
  CreateConfigResponse,
} from "./config.ts";

export type {
  RequestOtpBody,
  VerifyOtpBody,
  VerifyOtpResponse,
  MeResponse,
  MessageResponse,
} from "./auth.ts";
