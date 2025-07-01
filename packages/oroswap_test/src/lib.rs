#![cfg(not(tarpaulin_include))]

// Only re-export test dependencies when not building for WASM
#[cfg(not(target_arch = "wasm32"))]
pub use cw20_base;
#[cfg(not(target_arch = "wasm32"))]
pub use cw_multi_test;

pub mod coins;
pub mod convert;

// Only include test modules when not building for WASM
#[cfg(not(target_arch = "wasm32"))]
pub mod modules;
