#![macro_use]

use async_std::sync::{RwLock};
use std::any::*;
use std::sync::Arc;

use crate::entity::entity_manager::*;
use crate::entity::component_storage::*;

pub struct EntityComponentManager {
    stores: RwLock<rustc_hash::FxHashMap<TypeId, Box<dyn ComponentStoreAccessor + Sync + Send>>>,
    entities: Arc<RwLock<EntityManager>>,
}

impl Default for EntityComponentManager {
    fn default() -> Self {
        Self{
            stores: RwLock::new(rustc_hash::FxHashMap::default()),
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
    pub async fn register_component<T: 'static + Default + Clone + Component>(&self) {
        let type_id =  TypeId::of::<T>();
        let mut stores = self.stores.write().await;
        assert!(!stores.contains_key(&type_id), "Can not register Component multiple times.");
        if TypeId::of::<T::Storage>() == TypeId::of::<LinearStore<T>>() {
            stores.insert(type_id, Box::new(Arc::new(RwLock::new(LinearStore::<T>::new()))));
        }else if TypeId::of::<T::Storage>() == TypeId::of::<DenseStore<T>>() {
            stores.insert(type_id, Box::new(Arc::new(RwLock::new(DenseStore::<T>::new()))));
        } else {
            assert!(false, "missing arm in register component function");
        }
    }

    #[allow(unused)]
    pub async fn get_store<C: 'static + Default + Clone + Component>(&self) -> Arc<RwLock<C::Storage>> {
        let type_id =  TypeId::of::<C>();
        if let Some(arc_store) = self.stores.read().await.get(&&type_id) {
            return arc_store.as_any_ref().downcast_ref::<Arc<RwLock<C::Storage>>>().unwrap().clone();
        } 
        self.register_component::<C>().await;
        self.stores.read().await.get(&&type_id).unwrap().as_any_ref().downcast_ref::<Arc<RwLock<C::Storage>>>().unwrap().clone()
    }

    #[allow(unused)]
    pub fn get_entities(&self) -> Arc<RwLock<EntityManager>> {
        self.entities.clone()
    }

    #[allow(unused)]
    pub async fn cleanup(&mut self) {
        let mut entities = self.entities.write().await;
        {   
            let entities = &mut *entities;
            let destruct_queue = &*entities.entity_destruct_queue;
            let slots = &mut *entities.entity_slots;
            for ent in  destruct_queue {
                slots[*ent as usize].version += 1;
            }
    
            let mut stores = self.stores.write().await;
            for (_, store) in &mut*stores {
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

/**
 * Get exclusive reference to a component storage.
 * syntax:  (manager: my_entity_component_manager; components: ComponentTypes... => component_storage_reference_names...)
 */
#[allow(unused)]
#[macro_export]
macro_rules! get_components_mut {    
    (manager: $ecm:ident; components: $StoreType:ty $(,$($RestTypes:ty),+)? => $name:ident $(,$($RestNames:ident),+)?) => {
        let $name = $ecm.get_store::<$StoreType>().await;
        let mut $name = $name.write().await;
        let $name = &mut*$name;
        $(get_components_mut!(manager: $ecm; components: $($RestTypes),+ => $($RestNames),+))?
    };
}

/**
 * Get exclusive reference to a component storage.
 * syntax:  (manager: my_entity_component_manager; components: ComponentTypes... => component_storage_reference_names...)
 */
#[allow(unused)]
#[macro_export]
macro_rules! get_components {    
    (manager: $ecm:ident; components: $StoreType:ty $(,$($RestTypes:tt),+)? => $name:ident $(,$($RestNames:ident),+)?) => {
        let $name = $ecm.get_store::<$StoreType>().await;
        let $name = $name.read().await;
        let $name = &*$name;
        $(get_components!(manager: $ecm; components: $($RestTypes),+ => $($RestNames),+))?
    };
}

/**
 * Get shared reference to a component storage.
 * syntax:  (manager: component_manager_name => reference_name)
 */
#[allow(unused)]
#[macro_export]
macro_rules! get_entities {
    (manager: $ecm:ident => $name:ident) => {
        let $name = $ecm.get_entities();
        let $name = $name.read().await;
        let $name = &*$name;
    };
}

/**
 * Get exclusive reference to a component storage.
 * syntax:  (manager: component_manager_name => reference_name)
 */
#[allow(unused)]
#[macro_export]
macro_rules! get_entities_mut {
    (manager: $ecm:ident => $name:ident) => {
        let $name = $ecm.get_entities();
        let mut $name = $name.write().await;
        let $name = &mut*$name;
    };
}