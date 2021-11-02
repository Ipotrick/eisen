#![macro_use]

pub mod task;
mod runtime;
mod atomic_waiter;
mod block_on;
mod sleep;
mod yielding;

pub use runtime::*;
pub use atomic_waiter::*;
pub use block_on::*;
pub use sleep::*;
pub use yielding::*;

#[allow(unused)]
macro_rules! spin_on {
    ($expression:expr) => {
        loop {
            if let Some(guard) = $expression {
                break guard
            }
        }
    };
}