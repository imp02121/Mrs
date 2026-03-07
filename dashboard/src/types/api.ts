/** Standard single-item API response wrapper: `{ data: T }`. */
export interface ApiResponse<T> {
  data: T;
}

/** Paginated API response: `{ data: T[], pagination: Pagination }`. */
export interface PaginatedResponse<T> {
  data: T[];
  pagination: Pagination;
}

/** Pagination metadata returned with paginated responses. */
export interface Pagination {
  /** Current page number (0-indexed). */
  page: number;
  /** Items per page. */
  per_page: number;
  /** Total number of items across all pages. */
  total_items: number;
  /** Total number of pages. */
  total_pages: number;
}

/** Standard API error response. */
export interface ApiErrorResponse {
  error: ApiErrorDetail;
}

/** Error detail within an API error response. */
export interface ApiErrorDetail {
  /** Machine-readable error code. */
  code: string;
  /** Human-readable error message. */
  message: string;
  /** Optional additional details. */
  details: unknown;
}
