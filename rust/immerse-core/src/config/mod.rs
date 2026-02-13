//! Configuration loading and validation for environment definitions.

mod loader;
mod types;
mod validator;

pub use loader::{
    get_available_times, get_available_times_at_path, get_time_variant_engines,
    get_time_variant_engines_at_path, has_time_variants, has_time_variants_at_path,
    resolve_time_variant, ConfigLoader, TIME_PERIODS,
};
pub use types::*;
pub use validator::ConfigValidator;
