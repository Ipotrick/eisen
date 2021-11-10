use std::any::*;
use std::sync::Arc;

use crate::entity::handle::*;

mod dense_store;
pub use dense_store::*;

mod linear_store;
pub use linear_store::*;

pub trait GenericComponentStore {
    fn optimize(&mut self);

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn has(&self, index: EntityIndex) -> bool;

    fn rem(&mut self, index: EntityIndex);

    fn len(&self) -> usize;
}

pub trait ComponentStore<T: Default + Clone> {
    type ComponentType : Component;

    fn new() -> Self;

    fn get(&self, index: EntityIndex) -> Option<&T>;

    fn get_mut(&mut self, index: EntityIndex) -> Option<&mut T>;

    fn set(&mut self, index: EntityIndex, value: T);

    fn add(&mut self, index: EntityIndex, value: T);
}

pub trait Component : Clone + Default + Sync + Send
{
    type Storage : ComponentStore<Self>;
}