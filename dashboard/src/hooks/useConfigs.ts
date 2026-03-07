import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { CreateConfigRequest } from "@/types/index.ts";
import {
  listConfigs,
  getConfig,
  createConfig,
  deleteConfig,
} from "@/api/endpoints.ts";

/** Query: list all saved configs. */
export function useConfigs() {
  return useQuery({
    queryKey: ["configs"],
    queryFn: listConfigs,
  });
}

/** Query: fetch a single config by ID. */
export function useConfig(id: string | undefined) {
  return useQuery({
    queryKey: ["config", id],
    queryFn: () => getConfig(id!),
    enabled: !!id,
  });
}

/** Mutation: create a new config. Invalidates the configs list on success. */
export function useCreateConfig() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (req: CreateConfigRequest) => createConfig(req),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["configs"] });
    },
  });
}

/** Mutation: delete a config. Invalidates the configs list on success. */
export function useDeleteConfig() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteConfig(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["configs"] });
    },
  });
}
