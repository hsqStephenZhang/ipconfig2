pub mod adapter;
mod error;

pub mod computer;

pub use adapter::{get_adapters, Adapter, OperStatus, IfType};
