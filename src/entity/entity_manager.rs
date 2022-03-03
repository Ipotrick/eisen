use crate::entity::handle::*; 
use super::component_storage::*;

pub struct EntityManager {
    pub(crate) entity_slots: Vec<EntitySlot>,
    pub entity_free_list: Vec<EntityIndex>,
    pub entity_destruct_queue: Vec<EntityIndex>,
}

impl EntityManager {
    pub fn new() -> Self {
        Self{
            entity_slots: Vec::new(),
            entity_free_list: Vec::new(),
            entity_destruct_queue: Vec::new(),
        }
    }

    pub(crate) fn exists_index(&self, index: EntityIndex) -> bool {
        (index as usize) < self.entity_slots.len() && self.entity_slots[index as usize].alive
    }

    #[allow(unused)]
    pub fn version_of(&self, index: EntityIndex) -> Option<EntityVersion> {
        if self.exists_index(index) {
            Some(self.entity_slots[index as usize].version)
        } else {
            None
        }
    }

    pub fn exists(&self, entity: EntityHandle) -> bool {
        (entity.index as usize) < self.entity_slots.len() && self.entity_slots[entity.index as usize].alive && self.entity_slots[entity.index as usize].version == entity.version
    }

    #[allow(unused)]
    pub fn create(&mut self) -> EntityHandle {
        let index = self.entity_free_list.pop().unwrap_or(
            {
                let index = self.entity_slots.len();
                self.entity_slots.push(EntitySlot::new());
                index as EntityIndex
            }
        );
        let entity_slot = &mut self.entity_slots[index as usize];
        entity_slot.alive = true;
        EntityHandle{index:index, version:entity_slot.version}
    }

    #[allow(unused)]
    pub fn destroy(&mut self, entity: EntityHandle) {
        assert!(self.exists(entity));
        self.entity_slots[entity.index as usize].alive = false;
        self.entity_destruct_queue.push(entity.index);
    }

    #[allow(unused)]
    pub fn add<C: Component, Store: ComponentStore<C>>(&self, store: &mut Store, value: C, entity: EntityHandle) {
        assert!(self.exists(entity));
        store.add(entity.index, value);
    }

    #[allow(unused)]
    pub fn rem<C: Component, Store: ComponentStore<C> + GenericComponentStore>(&mut self, store: &mut Store, entity: EntityHandle) {
        assert!(self.exists(entity));
        store.rem(entity.index);
    }

    #[allow(unused)]
    pub fn has<C: Component, Store: ComponentStore<C> + GenericComponentStore>(&self, store: &Store, entity: EntityHandle) -> bool {
        assert!(self.exists(entity));
        store.has(entity.index)
    }

    #[allow(unused)]
    pub fn get<'c, C: Component, Store: ComponentStore<C> + GenericComponentStore>(&self, store: &'c Store, entity: EntityHandle) -> Option<&'c C> {
        assert!(self.exists(entity));
        store.get(entity.index)
    }

    #[allow(unused)]
    pub fn get_mut<'c, C: Component, Store: ComponentStore<C> + GenericComponentStore>(&self, store: &'c mut Store, entity: EntityHandle) -> Option<&'c mut C> {
        assert!(self.exists(entity));
        store.get_mut(entity.index)
    }
}