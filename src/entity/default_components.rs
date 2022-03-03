use crate::Vf32x2;

use super::{Component, DenseStore, LinearStore};

#[derive(Clone, Copy)]
pub struct Transform {
    pub position: cgmath::Vector2<f32>,
    pub orientation: cgmath::Vector2<f32>,
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

#[derive(Clone, Copy)]
pub struct OldTransform {
    pub position: cgmath::Vector2<f32>,
    pub orientation: cgmath::Vector2<f32>,
}

impl Default for OldTransform {
    fn default() -> Self {
        Self{
            position: Vf32x2::new(0.0, 0.0),
            orientation: Vf32x2::new(0.0, 0.0),
        }
    }
}

impl Component for OldTransform {
    type Storage = LinearStore<Self>;
}

#[derive(Clone, Copy)]
pub struct RectRenderable {
    pub size: cgmath::Vector2<f32>,
    pub color: cgmath::Vector4<f32>,
}

impl Default for RectRenderable {
    fn default() -> Self {
        Self{
            size: Vf32x2::new(1.0, 1.0),
            color: cgmath::Vector4::<f32>::new(1.0,1.0,1.0,1.0),
        }
    }
}

impl Component for RectRenderable {
    type Storage = DenseStore<Self>;
}