//! Standard API response wrappers.
//!
//! [`ApiResponse`] wraps a single value, [`PaginatedResponse`] wraps a list
//! with pagination metadata. Both serialize to JSON via Axum.

use axum::Json;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

/// Wrapper for single-item JSON responses: `{"data": T}`.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    /// The response payload.
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    /// Wrap a value in an API response.
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

/// Pagination metadata included in paginated responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// Current page number (0-indexed).
    pub page: u32,
    /// Items per page.
    pub per_page: u32,
    /// Total number of items across all pages.
    pub total_items: i64,
    /// Total number of pages.
    pub total_pages: u32,
}

impl Pagination {
    /// Compute pagination metadata from query parameters and total item count.
    pub fn from_query(page: u32, per_page: u32, total: i64) -> Self {
        let total_pages = if per_page == 0 {
            0
        } else {
            (total as u64).div_ceil(per_page as u64) as u32
        };
        Self {
            page,
            per_page,
            total_items: total,
            total_pages,
        }
    }
}

/// Paginated list response: `{"data": [...], "pagination": {...}}`.
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    /// The page of results.
    pub data: Vec<T>,
    /// Pagination metadata.
    pub pagination: Pagination,
}

impl<T: Serialize> PaginatedResponse<T> {
    /// Build a paginated response from items and pagination info.
    pub fn new(data: Vec<T>, pagination: Pagination) -> Self {
        Self { data, pagination }
    }
}

impl<T: Serialize> IntoResponse for PaginatedResponse<T> {
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

/// Query parameters for pagination: `?page=0&per_page=50`.
#[derive(Debug, Clone, Deserialize)]
pub struct PaginationParams {
    /// Page number (0-indexed). Defaults to 0.
    pub page: Option<u32>,
    /// Items per page. Defaults to 50.
    pub per_page: Option<u32>,
}

impl PaginationParams {
    /// Page number with default of 0.
    pub fn page(&self) -> u32 {
        self.page.unwrap_or(0)
    }

    /// Items per page with default of 50, clamped to 200 max.
    pub fn per_page(&self) -> u32 {
        self.per_page.unwrap_or(50).min(200)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_from_query_basic() {
        let p = Pagination::from_query(0, 25, 100);
        assert_eq!(p.page, 0);
        assert_eq!(p.per_page, 25);
        assert_eq!(p.total_items, 100);
        assert_eq!(p.total_pages, 4);
    }

    #[test]
    fn test_pagination_from_query_remainder() {
        let p = Pagination::from_query(0, 25, 101);
        assert_eq!(p.total_pages, 5);
    }

    #[test]
    fn test_pagination_from_query_zero_items() {
        let p = Pagination::from_query(0, 25, 0);
        assert_eq!(p.total_pages, 0);
        assert_eq!(p.total_items, 0);
    }

    #[test]
    fn test_pagination_from_query_zero_per_page() {
        let p = Pagination::from_query(0, 0, 100);
        assert_eq!(p.total_pages, 0);
    }

    #[test]
    fn test_pagination_params_defaults() {
        let params = PaginationParams {
            page: None,
            per_page: None,
        };
        assert_eq!(params.page(), 0);
        assert_eq!(params.per_page(), 50);
    }

    #[test]
    fn test_pagination_params_custom() {
        let params = PaginationParams {
            page: Some(3),
            per_page: Some(10),
        };
        assert_eq!(params.page(), 3);
        assert_eq!(params.per_page(), 10);
    }

    #[test]
    fn test_pagination_params_max_per_page() {
        let params = PaginationParams {
            page: None,
            per_page: Some(500),
        };
        assert_eq!(params.per_page(), 200);
    }

    #[test]
    fn test_api_response_serialization() {
        let resp = ApiResponse::new("hello");
        let json = serde_json::to_value(&resp).expect("serialize");
        assert_eq!(json["data"], "hello");
    }

    #[test]
    fn test_paginated_response_serialization() {
        let pagination = Pagination::from_query(0, 10, 25);
        let resp = PaginatedResponse::new(vec![1, 2, 3], pagination);
        let json = serde_json::to_value(&resp).expect("serialize");
        assert_eq!(json["data"].as_array().expect("array").len(), 3);
        assert_eq!(json["pagination"]["total_items"], 25);
        assert_eq!(json["pagination"]["total_pages"], 3);
    }

    #[test]
    fn test_pagination_single_item() {
        let p = Pagination::from_query(0, 10, 1);
        assert_eq!(p.total_pages, 1);
        assert_eq!(p.total_items, 1);
    }

    #[test]
    fn test_pagination_per_page_greater_than_total() {
        let p = Pagination::from_query(0, 100, 5);
        assert_eq!(p.total_pages, 1);
    }

    #[test]
    fn test_pagination_exact_multiple() {
        let p = Pagination::from_query(0, 50, 150);
        assert_eq!(p.total_pages, 3);
    }

    #[test]
    fn test_pagination_page1_per50_total342() {
        let p = Pagination::from_query(1, 50, 342);
        assert_eq!(p.total_pages, 7);
        assert_eq!(p.page, 1);
        assert_eq!(p.per_page, 50);
        assert_eq!(p.total_items, 342);
    }

    #[test]
    fn test_pagination_per_page_1() {
        let p = Pagination::from_query(0, 1, 5);
        assert_eq!(p.total_pages, 5);
    }

    #[test]
    fn test_pagination_params_clamped_at_200() {
        let params = PaginationParams {
            page: Some(0),
            per_page: Some(201),
        };
        assert_eq!(params.per_page(), 200);
    }

    #[test]
    fn test_pagination_params_exactly_200() {
        let params = PaginationParams {
            page: None,
            per_page: Some(200),
        };
        assert_eq!(params.per_page(), 200);
    }

    #[test]
    fn test_pagination_params_below_max() {
        let params = PaginationParams {
            page: None,
            per_page: Some(199),
        };
        assert_eq!(params.per_page(), 199);
    }

    #[test]
    fn test_api_response_with_nested_struct() {
        #[derive(Serialize)]
        struct Inner {
            x: i32,
            y: String,
        }
        let resp = ApiResponse::new(Inner {
            x: 42,
            y: "hello".into(),
        });
        let json = serde_json::to_value(&resp).expect("serialize");
        assert_eq!(json["data"]["x"], 42);
        assert_eq!(json["data"]["y"], "hello");
    }

    #[test]
    fn test_api_response_with_vec() {
        let resp = ApiResponse::new(vec![1, 2, 3]);
        let json = serde_json::to_value(&resp).expect("serialize");
        assert_eq!(json["data"].as_array().expect("array").len(), 3);
    }

    #[test]
    fn test_paginated_response_empty_vec() {
        let pagination = Pagination::from_query(0, 50, 0);
        let resp = PaginatedResponse::<String>::new(vec![], pagination);
        let json = serde_json::to_value(&resp).expect("serialize");
        assert!(json["data"].as_array().expect("array").is_empty());
        assert_eq!(json["pagination"]["total_items"], 0);
        assert_eq!(json["pagination"]["total_pages"], 0);
    }

    #[test]
    fn test_pagination_serde_roundtrip() {
        let p = Pagination::from_query(2, 25, 100);
        let json = serde_json::to_string(&p).expect("serialize");
        let parsed: Pagination = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.page, 2);
        assert_eq!(parsed.per_page, 25);
        assert_eq!(parsed.total_items, 100);
        assert_eq!(parsed.total_pages, 4);
    }
}
