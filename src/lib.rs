pub mod adapter;
pub mod error;

#[cfg(feature = "computer")]
pub mod computer;
pub mod dns;
pub mod fwpm;
pub mod utils;

pub mod ifindex;

pub use adapter::{get_adapters, Adapter, IfType, OperStatus};
pub use ifindex::{find_adapter_interface_index as if_nametoindex, set_ip_unicast_if};
