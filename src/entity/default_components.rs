use crate::Vf32x2;

use super::{Component, DenseStore, LinearStore};

#[derive(Clone)]
pub struct Transform {
    position: Vf32x2,
    orientation: Vf32x2,
}

impl Default for Transform {
    fn default() -> Self {
        Self{
            position: Vf32x2::new(0.0, 0.0),
            orientation: Vf32x2::new(0.0, 0.0),
        }
    }
}

impl Component for Transform {
    type Storage = LinearStore<Self>;
}

#[derive(Clone)]
pub struct RectRenderable {
    size: Vf32x2,
    color: cgmath::Vector4<u8>,
}

impl Default for RectRenderable {
    fn default() -> Self {
        Self{
            size: Vf32x2::new(1.0, 1.0),
            color: cgmath::Vector4::<u8>::new(255,255,255,255),
        }
    }
}

impl Component for RectRenderable {
    type Storage = DenseStore<Self>;
}