use async_std::sync::{RwLock};
use std::any::*;
use std::sync::Arc;

use crate::entity::entity_manager::*;
use crate::entity::component_storage::*;

pub struct EntityComponentManager {
    stores: rustc_hash::FxHashMap<TypeId, Box<dyn ComponentStoreAccessor + Sync + Send>>,
    entities: Arc<RwLock<EntityManager>>,
}

impl Default for EntityComponentManager {
    fn default() -> Self {
        Self{
            stores: rustc_hash::FxHashMap::default(),
            entities: Arc::new(RwLock::new(EntityManager::new())),
        }
    }
}

impl EntityComponentManager {

    #[allow(unused)]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(unused)]
    pub fn register_component<T: 'static + Default + Clone + Component>(&mut self) {
        let type_id =  TypeId::of::<T>();
        assert!(!self.stores.contains_key(&type_id), "Can not register Component multiple times.");
        if TypeId::of::<T::Storage>() == TypeId::of::<LinearStore<T>>() {
            self.stores.insert(type_id, Box::new(Arc::new(RwLock::new(LinearStore::<T>::new()))));
        }else if TypeId::of::<T::Storage>() == TypeId::of::<DenseStore<T>>() {
            self.stores.insert(type_id, Box::new(Arc::new(RwLock::new(DenseStore::<T>::new()))));
        } else {
            assert!(false, "missing arm in register component function");
        }
    }

    #[allow(unused)]
    pub async fn get_store<C: 'static + Default + Clone + Component>(&self) -> async_std::sync::RwLockReadGuard<'_, C::Storage> {
        let type_id =  TypeId::of::<C>();
        let store = self.stores.get(&&type_id).unwrap().as_any_ref().downcast_ref::<Arc<RwLock<C::Storage>>>().clone().unwrap();
        store.read().await
    }

    #[allow(unused)]
    pub async fn get_store_mut<C: 'static + Default + Clone + Component>(&self) -> async_std::sync::RwLockWriteGuard<'_, C::Storage> {
        let type_id =  TypeId::of::<C>();
        self.stores.get(&type_id).unwrap().as_any_ref().downcast_ref::<Arc<RwLock<C::Storage>>>().clone().unwrap().write().await
    }

    #[allow(unused)]
    pub async fn get_entities(&self) -> async_std::sync::RwLockReadGuard<'_, EntityManager> {
        self.entities.read().await
    }

    #[allow(unused)]
    pub async fn get_entities_mut(& self) -> async_std::sync::RwLockWriteGuard<'_, EntityManager> {
        self.entities.write().await
    }

    #[allow(unused)]
    pub fn cleanup(&mut self) {
        let mut entities = spin_on!(self.entities.try_write());
        {
            let entities = &mut *entities;
            let destruct_queue = &*entities.entity_destruct_queue;
            let slots = &mut *entities.entity_slots;
            for ent in  destruct_queue {
                slots[*ent as usize].version += 1;
            }
    
            for (_, store) in &mut self.stores {
                store.exec(&mut |store: &mut dyn GenericComponentStore| {
                    for index in destruct_queue {
                        if (&store).has(*index) { 
                            store.rem(*index) 
                        } 
                    }
                });
            }
        }

        while let Some(index) = entities.entity_destruct_queue.pop() {
            entities.entity_free_list.push(index);
        }
    }
} 