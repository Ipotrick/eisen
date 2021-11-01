#![macro_use]

pub mod handle;
pub mod entity_manager;
pub mod component_storage;
pub mod component_manager;
pub mod iteration;

#[allow(unused)]
pub use handle::{EntityHandle};
#[allow(unused)]
pub use component_storage::{DenseStore, LinearStore, Component};
pub(crate) use component_storage::{GenericComponentStore, ComponentStore};
#[allow(unused)]
pub use component_manager::{EntityComponentManager};
#[allow(unused)]
pub use entity_manager::{EntityManager};
#[allow(unused)]
pub use iteration::*;