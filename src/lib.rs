pub mod sync;
pub mod entity;
pub mod util;
pub mod app;

#[allow(unused)]
use sync::*;
#[allow(unused)]
use entity::{DenseStore,EntityHandle, GenericComponentStore, ComponentStore};
#[allow(unused)]
use util::*;
#[allow(unused)]
use app::*;

#[cfg(test)]
mod tests {

    #[allow(unused)]
    use crate::entity::{EntityComponentManager};

    use super::*;

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

    struct MyUser {

    }

    impl User for MyUser {
        fn init(self: Arc<Self>, _: Arc<AppData>) { println!("user init!"); }
        fn cleanup(self: Arc<Self>, _: Arc<AppData>)  { println!("user cleanup!"); }
        fn vary_tick(self: Arc<Self>, _: Arc<AppData>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>{ Box::pin(async{/*println!("user vary_tick!");*/}) }
        fn fixed_tick(self: Arc<Self>, _: Arc<AppData>, _: Arc<FixedMeta>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>{ Box::pin(async{/*println!("user fixed_tick!");*/}) }
    }

    #[test]
    fn dev() {
        let my_user = MyUser{};
        let app = Application::new(my_user);
        app.run();
    }

    #[test]
    fn test2() {
        let _runtime = Runtime::new();
    }

    #[test]
    fn ecm_par_iter_works() {
        let runtime = Runtime::new();
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
        let mut entities2 = block_on(ecm.get_entities_mut());
        let entities = &mut*entities2;

        const N: usize = 1_000_000;

        for i in 0..N {
            let entity = entities.create();
            let name = String::from("Entity Nr.") + i.to_string().as_str();
            entities.add(names, Name(name.clone()), entity);
            if i > N/2 {
                entities.add(positions, Pos(0.0,0.0), entity);
            }
            if i % 4 == 0 {
                entities.add(healths, Health(i as u32 + 1), entity);
            }
        }

        let before = std::time::SystemTime::now();
        block_on(parallel_over_entities!( 
            runtime: runtime;
            batch_size: 200; 
            closure: |(_, health, name) : (EntityHandle, &mut Health, &mut Name)|
            {
                health.0 += 1;
                let c = name.0.to_uppercase();
                let n = c.len();
                health.0 += f32::sqrt(n as f32) as u32;
                assert!(health.0 < 1_000_000);
            };
            entities: entities; 
            stores: mut healths, not positions, mut names
        ));

        let past = std::time::SystemTime::now().duration_since(before).unwrap();
        println!("time taken: {} mics", past.as_micros());
        println!("past!");
        assert!(true);
    }

    #[test]
    fn runtime_works() {
        let rt = crate::sync::Runtime::new();
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
                rt.spawn_prioritised(task, sync::task::Priority::Low);
            }
            for _ in 0..100 {
                let d = d.clone();
                let counter = counter.clone();
                let task = async move { 
                    let _dep = d.clone();
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    println!("high priority task");
                };
                rt.spawn_prioritised(task, sync::task::Priority::High);
            }
            for _ in 0..100 {
                let d = d.clone();
                let counter = counter.clone();
                let task = async move { 
                    let _dep = d.clone();
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    println!("very high priority task");
                };
                rt.spawn_prioritised(task, sync::task::Priority::VeryHigh);
            }
    
            w2
        };
    
        block_on(w);
    
        println!("end");
        
        assert_eq!(counter.fetch_add(0, std::sync::atomic::Ordering::Relaxed), 400);
    }
}
