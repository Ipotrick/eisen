mod runtime;
#[allow(unused)]
use runtime::*;
mod sync;
#[allow(unused)]
use sync::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
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
    
        rt.block_on(w);
    
        println!("end");
        
        assert_eq!(counter.fetch_add(0, std::sync::atomic::Ordering::Relaxed), 400);
    }
}
