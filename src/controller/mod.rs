pub mod root;
pub use builder::RoutingMode;
pub use handle::ControllerHandle;
pub use root::*;

mod builder;
mod handle;
#[cfg(test)]
mod ipip_tests;
