pub mod runtime;
pub mod sync;
pub mod entity;
pub mod util;

#[allow(unused)]
use runtime::*;
#[allow(unused)]
use sync::*;
#[allow(unused)]
use entity::{DenseStore,EntityHandle, GenericComponentStore, ComponentStore};
#[allow(unused)]
use util::*;

#[derive(Clone,Default)]
struct Health(u32);

impl entity::Component for Health {
    type Storage = entity::DenseStore<Self>;
} 

#[derive(Clone,Default)]
struct Pos(f32,f32);

impl entity::Component for Pos {
    type Storage = entity::DenseStore<Self>;
}

#[derive(Clone,Default)]
struct Name(String);

impl entity::Component for Name {
    type Storage = entity::DenseStore<Self>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn ecm_not_works() {
        let mut ecm = entity::EntityComponentManager::new();
        ecm.register_component::<Health>();
        ecm.register_component::<Pos>();
        ecm.register_component::<Name>();

        let mut healths = block_on(ecm.get_store_mut::<Health>());
        let healths = &mut*healths;
        let mut positions = block_on(ecm.get_store_mut::<Pos>());
        let positions = &mut*positions;
        let mut names = block_on(ecm.get_store_mut::<Name>());
        let names = &mut*names;
        let mut entities = block_on(ecm.get_entities_mut());
        let entities = &mut*entities;

        for i in 0..100 {
            let entity = entities.create();
            let name = String::from("Entity Nr.") + i.to_string().as_str();
            entities.add(names, Name(name), entity);
            if i > 49 {
                entities.add(positions, Pos(0.0,0.0), entity);
            }
            if i % 4 == 0 {
                entities.add(healths, Health(i + 1), entity);
            }
        }

        for (health,) in iterate_over_components!(mut healths) {
            health.0 -= 1;
        }
    }

    #[test]
    fn runtime_works() {
        let rt = runtime::Runtime::new();
        let counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let w = {
            let w = AtomicWaiter::new();
            let w2 = AtomicWaiter::new();
            let d = w2.make_dependency();
    
            let d2 = d.clone();
            rt.spawn(async move { 
                let _dep = d2.clone(); 
                w.await;
                println!("I Woke up!");
            });
            for _ in 0..100 {
                let d = d.clone();
                let counter = counter.clone();
                let task = async move { 
                    let _dep = d.clone();
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    //sleep_for_task(std::time::Duration::from_secs(1)).await;
                    println!("normal priority task");
                };
                rt.spawn(task);
            }
            for _ in 0..100 {
                let d = d.clone();
                let counter = counter.clone();
                let task = async move { 
                    let _dep = d.clone();
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    println!("low priority task");
                };
                rt.spawn_prioritised(task, runtime::task::Priority::Low);
            }
            for _ in 0..100 {
                let d = d.clone();
                let counter = counter.clone();
                let task = async move { 
                    let _dep = d.clone();
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    println!("high priority task");
                };
                rt.spawn_prioritised(task, runtime::task::Priority::High);
            }
            for _ in 0..100 {
                let d = d.clone();
                let counter = counter.clone();
                let task = async move { 
                    let _dep = d.clone();
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    println!("very high priority task");
                };
                rt.spawn_prioritised(task, runtime::task::Priority::VeryHigh);
            }
    
            w2
        };
    
        block_on(w);
    
        println!("end");
        
        assert_eq!(counter.fetch_add(0, std::sync::atomic::Ordering::Relaxed), 400);
    }
}
