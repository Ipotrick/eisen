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

    use std::time::Instant;

    use futures::executor::block_on;

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
        type Storage = entity::LinearStore<Self>;
    }
    
    #[derive(Clone,Default)]
    struct Name(String);
    
    impl entity::Component for Name {
        type Storage = entity::DenseStore<Self>;
    }

    struct MyUser {

    }

    //fn operation((_, health) : (EntityHandle, &mut Health)) {
    //    health.0 += 1;
    //}

    impl User for MyUser {
        fn init(self: Arc<Self>, app: Arc<AppData>) {
            let init = async{
                println!("user init!"); 
                let ecm = &app.ecm;
                
                get_components_mut!(manager: ecm; components: Health, Pos, Name => healths, positions, names);
                get_entities_mut!(manager: ecm => entities);
        
                const N: usize = 100_000;
                for i in 0..N {
                    let entity = entities.create();
    
                    entities.add(names, Name(String::from("Entity Nr.") + i.to_string().as_str()), entity);
    
                    if i > N/2 {
                        entities.add(positions, Pos(0.0,0.0), entity);
                    }
    
                    if i % 4 == 0 {
                        entities.add(healths, Health(0), entity);
                    }
                }
                println!("init complete");
            } ;


            block_on(init);
        }
        fn cleanup(self: Arc<Self>, _: Arc<AppData>)  { println!("user cleanup!"); }

        fn vary_tick(self: Arc<Self>, app: Arc<AppData>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>{ 
            let app = app.clone();
            Box::pin(async move {

                let ecm = &app.ecm;
                //let runtime = &app.runtime;
                get_components_mut!(manager: ecm; components: Health => healths);
                get_entities_mut!(manager: ecm => entities);

                println!("before");
                let earlier = std::time::Instant::now();

                {
                    profiling::scope!("signal for start!");
                }
                //parallel_over_entities!(
                //    runtime: runtime;
                //    batch_size: 1000; 
                //    closure: 
                //    |(_, health) : (EntityHandle, &mut Health)|{
                //        health.0 = f32::exp2(f32::sqrt(health.0 as f32 * 3.3)) as u32;
                //    };
                //    entities: entities;
                //    stores: mut healths 
                //).await;
                {
                    profiling::scope!("sequential iter");
                    for (_, health) in iterate_over_entities!(entities: entities; stores: mut healths) {
                        health.0 = f32::exp2(f32::sqrt(health.0 as f32 * 3.3)) as u32;
                    }
                }
                {
                    profiling::scope!("signal for resume!");
                }


                println!("time taken: {}mics", earlier.elapsed().as_micros());
                println!("after");

                //app.end();
            })
        }
        fn fixed_tick(self: Arc<Self>, _: Arc<AppData>, _: Arc<FixedData>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>{ Box::pin(async{
            profiling::scope!("fixed_tick_user","");
            spin_sleep::sleep(std::time::Duration::from_millis(100));
        }) }
    }

    #[test]
    fn dev() {
        let my_user = MyUser{};
        let app = Application::new(my_user);
        app.run();
    }


    #[test]
    fn sleeptest() {
        let joiner = std::thread::spawn(||{
            loop {
                let earlier = Instant::now();
                spin_sleep::sleep(std::time::Duration::from_nanos(1_200_000));
                println!("sleep time taken: {}mics", earlier.elapsed().as_micros());
            }
        });
        joiner.join();
    }

    #[test]
    fn ecm_par_iter_works() {
        let runtime = Arc::new(Runtime::new());
        let waiter = sync::AtomicWaiter::new();
        let dep = waiter.make_dependency();

        let rt_clone = runtime.clone();
        let main_task = async{
            let _d = dep;
            let runtime = rt_clone;
            let ecm = entity::EntityComponentManager::new();

            get_components_mut!(manager: ecm; components: Health, Pos, Name => healths, positions, names);
            get_entities_mut!(manager: ecm => entities);
    
            const N: usize = 100_000;
            for i in 0..N {
                let entity = entities.create();

                entities.add(names, Name(String::from("Entity Nr.") + i.to_string().as_str()), entity);

                if i > N/2 {
                    entities.add(positions, Pos(0.0,0.0), entity);
                }

                if i % 4 == 0 {
                    entities.add(healths, Health(i as u32 + 1), entity);
                }
            }

            let before = std::time::SystemTime::now();

            println!("init finish");

            parallel_over_entities!( 
                note: "update note";
                runtime: runtime;
                batch_size: 200; 
                closure: 
                |(_, health, name) : (EntityHandle, &mut Health, &mut Name)|
                {
                    health.0 += 1;
                    let c = name.0.to_uppercase();
                    let n = c.len();
                    health.0 += f32::sqrt(n as f32) as u32;
                    //assert!(health.0 < 1_000_000);
                };
                entities: entities;
                stores: mut healths, mut names, not positions
            ).await;
    
            let past = std::time::SystemTime::now().duration_since(before).unwrap();
            println!("time taken: {} mics", past.as_micros());
            assert!(true);
        };


        runtime.spawn_prioritised(main_task, sync::task::Priority::VeryHigh);
        block_on(waiter);
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
