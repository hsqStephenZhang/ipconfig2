pub mod adapter;
pub mod error;

#[cfg(feature = "computer")]
pub mod computer;
pub mod dns;
pub mod fwpm;
pub mod utils;

pub use adapter::{get_adapters, Adapter, IfType, OperStatus};
