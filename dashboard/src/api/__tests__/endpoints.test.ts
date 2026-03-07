import { describe, it, expect, vi, beforeEach } from "vitest";
import type { AxiosInstance } from "axios";

// Mock both axios clients before importing endpoints
vi.mock("@/api/client.ts", () => {
  const mockApi = {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  };
  return { default: mockApi };
});

vi.mock("@/api/auth-client.ts", () => {
  const mockAuthApi = {
    get: vi.fn(),
    post: vi.fn(),
  };
  return { default: mockAuthApi };
});

describe("API endpoints", () => {
  let api: AxiosInstance;
  let authApi: AxiosInstance;

  beforeEach(async () => {
    vi.clearAllMocks();
    const clientMod = await import("@/api/client.ts");
    api = clientMod.default as unknown as AxiosInstance;
    const authMod = await import("@/api/auth-client.ts");
    authApi = authMod.default as unknown as AxiosInstance;
  });

  describe("Backtest endpoints", () => {
    it("should call POST /backtest/run for runBacktest", async () => {
      const mockResponse = {
        data: {
          data: {
            run_id: "test-id",
            result: {},
            duration_ms: 100,
          },
        },
      };
      vi.mocked(api.post).mockResolvedValueOnce(mockResponse);

      const { runBacktest } = await import("@/api/endpoints.ts");
      const req = {
        instrument: "DAX",
        start_date: "2024-01-01",
        end_date: "2024-12-31",
        config: {} as never,
      };
      const result = await runBacktest(req);

      expect(api.post).toHaveBeenCalledWith("/backtest/run", req);
      expect(result.run_id).toBe("test-id");
    });

    it("should call GET /backtest/:id for getBacktest", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({
        data: { data: { id: "abc" } },
      });

      const { getBacktest } = await import("@/api/endpoints.ts");
      const result = await getBacktest("abc");

      expect(api.get).toHaveBeenCalledWith("/backtest/abc");
      expect(result).toEqual({ id: "abc" });
    });

    it("should call GET /backtest/:id/trades with pagination", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({
        data: {
          data: [],
          pagination: { page: 1, per_page: 25, total_items: 0, total_pages: 0 },
        },
      });

      const { getBacktestTrades } = await import("@/api/endpoints.ts");
      await getBacktestTrades("abc", 1, 25);

      expect(api.get).toHaveBeenCalledWith("/backtest/abc/trades", {
        params: { page: 1, per_page: 25 },
      });
    });

    it("should use default pagination for getBacktestTrades", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({
        data: {
          data: [],
          pagination: { page: 0, per_page: 50, total_items: 0, total_pages: 0 },
        },
      });

      const { getBacktestTrades } = await import("@/api/endpoints.ts");
      await getBacktestTrades("xyz");

      expect(api.get).toHaveBeenCalledWith("/backtest/xyz/trades", {
        params: { page: 0, per_page: 50 },
      });
    });

    it("should call POST /backtest/compare for compareBacktests", async () => {
      vi.mocked(api.post).mockResolvedValueOnce({
        data: { data: [] },
      });

      const { compareBacktests } = await import("@/api/endpoints.ts");
      const req = { configs: [] };
      await compareBacktests(req);

      expect(api.post).toHaveBeenCalledWith("/backtest/compare", req);
    });

    it("should call GET /backtest/history with pagination", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({
        data: {
          data: [],
          pagination: { page: 0, per_page: 50, total_items: 0, total_pages: 0 },
        },
      });

      const { getBacktestHistory } = await import("@/api/endpoints.ts");
      await getBacktestHistory(2, 10);

      expect(api.get).toHaveBeenCalledWith("/backtest/history", {
        params: { page: 2, per_page: 10 },
      });
    });
  });

  describe("Config endpoints", () => {
    it("should call GET /configs for listConfigs", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({
        data: { data: [] },
      });

      const { listConfigs } = await import("@/api/endpoints.ts");
      await listConfigs();

      expect(api.get).toHaveBeenCalledWith("/configs");
    });

    it("should call GET /configs/:id for getConfig", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({
        data: { data: { id: "cfg-1", name: "Test", params: {}, created_at: "" } },
      });

      const { getConfig } = await import("@/api/endpoints.ts");
      const result = await getConfig("cfg-1");

      expect(api.get).toHaveBeenCalledWith("/configs/cfg-1");
      expect(result.id).toBe("cfg-1");
    });

    it("should call POST /configs for createConfig", async () => {
      vi.mocked(api.post).mockResolvedValueOnce({
        data: { data: { id: "cfg-new" } },
      });

      const { createConfig } = await import("@/api/endpoints.ts");
      const req = { name: "New Config", params: {} };
      const result = await createConfig(req);

      expect(api.post).toHaveBeenCalledWith("/configs", req);
      expect(result.id).toBe("cfg-new");
    });

    it("should call DELETE /configs/:id for deleteConfig", async () => {
      vi.mocked(api.delete).mockResolvedValueOnce({});

      const { deleteConfig } = await import("@/api/endpoints.ts");
      await deleteConfig("cfg-1");

      expect(api.delete).toHaveBeenCalledWith("/configs/cfg-1");
    });
  });

  describe("Data endpoints", () => {
    it("should call GET /data/instruments for listInstruments", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({
        data: { data: [] },
      });

      const { listInstruments } = await import("@/api/endpoints.ts");
      await listInstruments();

      expect(api.get).toHaveBeenCalledWith("/data/instruments");
    });

    it("should call GET /data/candles with params for getCandles", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({
        data: { data: [] },
      });

      const { getCandles } = await import("@/api/endpoints.ts");
      const params = { instrument: "DAX", from: "2024-01-01", to: "2024-12-31" };
      await getCandles(params);

      expect(api.get).toHaveBeenCalledWith("/data/candles", { params });
    });
  });

  describe("Signal endpoints", () => {
    it("should call GET /signals/today for getTodaySignals", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({
        data: { data: [] },
      });

      const { getTodaySignals } = await import("@/api/endpoints.ts");
      await getTodaySignals();

      expect(api.get).toHaveBeenCalledWith("/signals/today");
    });

    it("should call GET /signals/:instrument/latest for getLatestSignal", async () => {
      vi.mocked(api.get).mockResolvedValueOnce({
        data: { data: { id: "sig-1" } },
      });

      const { getLatestSignal } = await import("@/api/endpoints.ts");
      await getLatestSignal("DAX");

      expect(api.get).toHaveBeenCalledWith("/signals/DAX/latest");
    });
  });

  describe("Auth endpoints", () => {
    it("should call POST /auth/request-otp for requestOtp", async () => {
      vi.mocked(authApi.post).mockResolvedValueOnce({
        data: { message: "OTP sent" },
      });

      const { requestOtp } = await import("@/api/endpoints.ts");
      const result = await requestOtp({ email: "test@example.com" });

      expect(authApi.post).toHaveBeenCalledWith("/auth/request-otp", {
        email: "test@example.com",
      });
      expect(result.message).toBe("OTP sent");
    });

    it("should call POST /auth/verify-otp for verifyOtp", async () => {
      vi.mocked(authApi.post).mockResolvedValueOnce({
        data: { token: "jwt-token", expires_at: "2024-02-01T00:00:00Z" },
      });

      const { verifyOtp } = await import("@/api/endpoints.ts");
      const result = await verifyOtp({
        email: "test@example.com",
        otp: "123456",
      });

      expect(authApi.post).toHaveBeenCalledWith("/auth/verify-otp", {
        email: "test@example.com",
        otp: "123456",
      });
      expect(result.token).toBe("jwt-token");
    });

    it("should call GET /auth/me for getMe", async () => {
      vi.mocked(authApi.get).mockResolvedValueOnce({
        data: { email: "test@example.com", role: "admin" },
      });

      const { getMe } = await import("@/api/endpoints.ts");
      const result = await getMe();

      expect(authApi.get).toHaveBeenCalledWith("/auth/me");
      expect(result.email).toBe("test@example.com");
    });

    it("should call POST /auth/logout for postLogout", async () => {
      vi.mocked(authApi.post).mockResolvedValueOnce({});

      const { postLogout } = await import("@/api/endpoints.ts");
      await postLogout();

      expect(authApi.post).toHaveBeenCalledWith("/auth/logout");
    });
  });
});
