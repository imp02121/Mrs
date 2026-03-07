import { useQuery } from "@tanstack/react-query";
import type { CandleQuery } from "@/types/index.ts";
import { listInstruments, getCandles } from "@/api/endpoints.ts";

/** Query: list all available instruments. */
export function useInstruments() {
  return useQuery({
    queryKey: ["instruments"],
    queryFn: listInstruments,
  });
}

/** Query: fetch candle data. Only enabled when all params are set. */
export function useCandles(params: CandleQuery | null) {
  return useQuery({
    queryKey: ["candles", params],
    queryFn: () => getCandles(params!),
    enabled: params !== null,
  });
}
