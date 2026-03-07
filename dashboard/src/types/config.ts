/** A strategy configuration row. Mirrors Rust `ConfigResponse`. */
export interface ConfigRow {
  /** UUID string */
  id: string;
  name: string;
  params: unknown;
  /** ISO 8601 UTC timestamp */
  created_at: string;
}

/** Full config response matching Rust `ConfigResponse`. */
export interface ConfigResponse {
  /** UUID string */
  id: string;
  name: string;
  params: unknown;
  /** ISO 8601 UTC timestamp */
  created_at: string;
}

/** Request body for `POST /api/configs`. Mirrors Rust `CreateConfigRequest`. */
export interface CreateConfigRequest {
  name: string;
  params: unknown;
}

/** Response for a newly created config. Mirrors Rust `CreateConfigResponse`. */
export interface CreateConfigResponse {
  /** UUID string */
  id: string;
}
