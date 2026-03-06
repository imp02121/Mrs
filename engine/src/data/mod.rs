//! Data ingestion: provider traits, API fetchers, and local storage.

pub mod error;
pub mod fetcher;
pub mod parquet_store;
pub mod postgres_store;
pub mod provider;
pub mod twelve_data;

pub use error::DataError;
pub use fetcher::DataFetcher;
pub use parquet_store::ParquetStore;
pub use postgres_store::PostgresStore;
pub use provider::DataProvider;
pub use twelve_data::TwelveDataProvider;
