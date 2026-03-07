import { useQuery } from "@tanstack/react-query";
import { getTodaySignals, getLatestSignal } from "@/api/endpoints.ts";

/** Query: today's signals across all instruments. Auto-refreshes every 60s. */
export function useTodaySignals() {
  return useQuery({
    queryKey: ["signals", "today"],
    queryFn: getTodaySignals,
    refetchInterval: 60_000,
  });
}

/** Query: latest signal for a specific instrument. */
export function useLatestSignal(instrument: string | undefined) {
  return useQuery({
    queryKey: ["signals", "latest", instrument],
    queryFn: () => getLatestSignal(instrument!),
    enabled: !!instrument,
  });
}
