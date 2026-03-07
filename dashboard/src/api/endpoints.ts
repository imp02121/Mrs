import type {
  ApiResponse,
  PaginatedResponse,
  BacktestRunResponse,
  BacktestRunSummary,
  CompareRequest,
  CompareResultItem,
  RunBacktestRequest,
  ConfigResponse,
  CreateConfigRequest,
  CreateConfigResponse,
  InstrumentRow,
  CandleRow,
  CandleQuery,
  SignalRow,
  MeResponse,
  MessageResponse,
  VerifyOtpBody,
  VerifyOtpResponse,
  RequestOtpBody,
} from "@/types/index.ts";
import api from "./client.ts";
import authApi from "./auth-client.ts";

// ---------------------------------------------------------------------------
// Backtest
// ---------------------------------------------------------------------------

export async function runBacktest(
  req: RunBacktestRequest,
): Promise<BacktestRunResponse> {
  const res = await api.post<ApiResponse<BacktestRunResponse>>(
    "/backtest/run",
    req,
  );
  return res.data.data;
}

export async function getBacktest(id: string): Promise<unknown> {
  const res = await api.get<ApiResponse<unknown>>(`/backtest/${id}`);
  return res.data.data;
}

export async function getBacktestTrades(
  id: string,
  page = 0,
  perPage = 50,
): Promise<PaginatedResponse<unknown>> {
  const res = await api.get<PaginatedResponse<unknown>>(
    `/backtest/${id}/trades`,
    { params: { page, per_page: perPage } },
  );
  return res.data;
}

export async function compareBacktests(
  req: CompareRequest,
): Promise<CompareResultItem[]> {
  const res = await api.post<ApiResponse<CompareResultItem[]>>(
    "/backtest/compare",
    req,
  );
  return res.data.data;
}

export async function getBacktestHistory(
  page = 0,
  perPage = 50,
): Promise<PaginatedResponse<BacktestRunSummary>> {
  const res = await api.get<PaginatedResponse<BacktestRunSummary>>(
    "/backtest/history",
    { params: { page, per_page: perPage } },
  );
  return res.data;
}

// ---------------------------------------------------------------------------
// Configs
// ---------------------------------------------------------------------------

export async function listConfigs(): Promise<ConfigResponse[]> {
  const res = await api.get<ApiResponse<ConfigResponse[]>>("/configs");
  return res.data.data;
}

export async function getConfig(id: string): Promise<ConfigResponse> {
  const res = await api.get<ApiResponse<ConfigResponse>>(`/configs/${id}`);
  return res.data.data;
}

export async function createConfig(
  req: CreateConfigRequest,
): Promise<CreateConfigResponse> {
  const res = await api.post<ApiResponse<CreateConfigResponse>>(
    "/configs",
    req,
  );
  return res.data.data;
}

export async function deleteConfig(id: string): Promise<void> {
  await api.delete(`/configs/${id}`);
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

export async function listInstruments(): Promise<InstrumentRow[]> {
  const res = await api.get<ApiResponse<InstrumentRow[]>>(
    "/data/instruments",
  );
  return res.data.data;
}

export async function getCandles(
  params: CandleQuery,
): Promise<CandleRow[]> {
  const res = await api.get<ApiResponse<CandleRow[]>>("/data/candles", {
    params,
  });
  return res.data.data;
}

// ---------------------------------------------------------------------------
// Signals
// ---------------------------------------------------------------------------

export async function getTodaySignals(): Promise<SignalRow[]> {
  const res = await api.get<ApiResponse<SignalRow[]>>("/signals/today");
  return res.data.data;
}

export async function getLatestSignal(
  instrument: string,
): Promise<SignalRow> {
  const res = await api.get<ApiResponse<SignalRow>>(
    `/signals/${instrument}/latest`,
  );
  return res.data.data;
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

export async function requestOtp(
  body: RequestOtpBody,
): Promise<MessageResponse> {
  const res = await authApi.post<MessageResponse>(
    "/auth/request-otp",
    body,
  );
  return res.data;
}

export async function verifyOtp(
  body: VerifyOtpBody,
): Promise<VerifyOtpResponse> {
  const res = await authApi.post<VerifyOtpResponse>(
    "/auth/verify-otp",
    body,
  );
  return res.data;
}

export async function getMe(): Promise<MeResponse> {
  const res = await authApi.get<MeResponse>("/auth/me");
  return res.data;
}

export async function postLogout(): Promise<void> {
  await authApi.post("/auth/logout");
}
