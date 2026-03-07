import { useMutation, useQuery } from "@tanstack/react-query";
import type { RunBacktestRequest, CompareRequest } from "@/types/index.ts";
import {
  runBacktest,
  getBacktest,
  getBacktestTrades,
  compareBacktests,
  getBacktestHistory,
} from "@/api/endpoints.ts";

/** Mutation: run a single backtest. */
export function useRunBacktest() {
  return useMutation({
    mutationFn: (req: RunBacktestRequest) => runBacktest(req),
  });
}

/** Query: fetch a backtest result by ID. */
export function useBacktest(id: string | undefined) {
  return useQuery({
    queryKey: ["backtest", id],
    queryFn: () => getBacktest(id!),
    enabled: !!id,
  });
}

/** Query: fetch paginated trades for a backtest run. */
export function useBacktestTrades(
  id: string | undefined,
  page = 0,
  perPage = 50,
) {
  return useQuery({
    queryKey: ["backtest-trades", id, page, perPage],
    queryFn: () => getBacktestTrades(id!, page, perPage),
    enabled: !!id,
  });
}

/** Mutation: compare 2-4 backtest configurations. */
export function useCompareBacktests() {
  return useMutation({
    mutationFn: (req: CompareRequest) => compareBacktests(req),
  });
}

/** Query: fetch paginated backtest history. */
export function useBacktestHistory(page = 0, perPage = 50) {
  return useQuery({
    queryKey: ["backtest-history", page, perPage],
    queryFn: () => getBacktestHistory(page, perPage),
  });
}
