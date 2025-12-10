pub mod config;
pub mod routes;
pub mod services;
pub mod utils;
pub mod models;

// Re-exports
pub use utils::errors::Error;

// Result type alias
pub type Result<T> = std::result::Result<T, Error>;