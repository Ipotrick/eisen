pub type EntityIndex = u32;
pub type EntityVersion = u32;

#[derive(Clone, Copy)]
pub struct EntityHandle {
    pub(crate) index: EntityIndex,
    pub(crate) version: EntityVersion,
}

#[derive(Clone, Copy)]
pub(crate) struct EntitySlot {
    pub(crate) version: EntityVersion,
    pub(crate) alive: bool,
}

impl EntitySlot {
    pub(crate) fn new() -> Self {
        Self{
            version: 0,
            alive: false,
        }
    }
}