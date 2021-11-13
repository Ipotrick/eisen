pub use std::{cell::RefCell, pin::Pin, task::Waker};
pub use futures::{Future, FutureExt, task::{waker_ref}};
pub use smallbox::SmallBox;
use std::{sync::atomic::{AtomicU64, Ordering}};

use super::yielding::*;

pub use {
    futures::{
        task::{ArcWake},
    },
    std::{
        sync::{Arc, Mutex},
        task::{Context, Poll},
    },
};

use super::task::*;

pub(crate) enum RuntimeInfo {
    WakeUp
}

pub(crate) struct RuntimeMeta {
    worker_count: AtomicU64,
    execution_sender_low: crossbeam_channel::Sender<ExecutionOrder>,
    execution_sender_normal: crossbeam_channel::Sender<ExecutionOrder>,
    execution_sender_high: crossbeam_channel::Sender<ExecutionOrder>,
    execution_sender_very_high: crossbeam_channel::Sender<ExecutionOrder>,
    execution_reciever_low: crossbeam_channel::Receiver<ExecutionOrder>,
    execution_reciever_normal: crossbeam_channel::Receiver<ExecutionOrder>,
    execution_reciever_high: crossbeam_channel::Receiver<ExecutionOrder>,
    execution_reciever_very_high: crossbeam_channel::Receiver<ExecutionOrder>,
    signal_sender: crossbeam_channel::Sender<RuntimeInfo>,
    signal_reciever: crossbeam_channel::Receiver<RuntimeInfo>,
    pub(crate) end_runtime: std::sync::atomic::AtomicBool,
    pub(crate) open_tasks: std::sync::atomic::AtomicU64,
}

#[allow(unused)]
fn process_task(meta: &Arc<RuntimeMeta>, execution_sender: &crossbeam_channel::Sender<ExecutionOrder>, task: Arc<Task>) {
    let task_finished = {
        let waker = waker_ref(&task);
        let context = &mut Context::from_waker(&*waker);
    
        let finished = Poll::Pending != task.future.lock()
            .unwrap()
            .as_mut()
            .poll(context);
        
        YIELD_INFO.with(|info| {
            if info.borrow_mut().did_yield {
                info.borrow_mut().did_yield = false;
                info.borrow_mut().yield_task = Some(task);
            }
        });

        if finished {
            meta.open_tasks.fetch_sub(1, Ordering::AcqRel);
        }
        
        finished
    };
}

#[allow(unused)]
fn worker(meta: Arc<RuntimeMeta>, worker_index: usize) {
    
    // main worker loop
    'outer: while !meta.end_runtime.load(Ordering::Relaxed) || meta.open_tasks.load(Ordering::Acquire) > 0 {
        // if there are no tasks directly available, we sleep and wake up to execute the next available Order from any queue
        
        let (mut sender, mut order) = {
            crossbeam_channel::select! {
                recv(meta.execution_reciever_low) -> order => (&meta.execution_sender_low, order.unwrap()),
                recv(meta.execution_reciever_normal) -> order => (&meta.execution_sender_normal, order.unwrap()),
                recv(meta.execution_reciever_high) -> order => (&meta.execution_sender_high, order.unwrap()),
                recv(meta.execution_reciever_very_high) -> order => (&meta.execution_sender_very_high, order.unwrap()),
                recv(meta.signal_reciever) -> info => {
                    match info.unwrap() {
                        RuntimeInfo::WakeUp => {
                            continue 'outer;
                        }
                    }
                },
            }
        };

        let mut order = Some(order);

        'inner: while order.is_some() {
            {
                profiling::scope!("worker does work");
                match order.take().unwrap() {
                    ExecutionOrder::ExecuteTask(task) => {
                        process_task(&meta, sender, task);
                    },
                    ExecutionOrder::ExecuteClosure(mut closure) => {
                        closure();
                    },
                };
            };

            profiling::scope!("worker pick new work");

            // try to find new work:
            if let Ok(new_order) = meta.execution_reciever_very_high.try_recv() {
                order = Some(new_order);
                sender = &meta.execution_sender_very_high;
            }
            else if let Ok(new_order) = meta.execution_reciever_high.try_recv() {
                order = Some(new_order);
                sender = &meta.execution_sender_high;
            }
            else if let Ok(new_order) = meta.execution_reciever_normal.try_recv() {
                order = Some(new_order);
                sender = &meta.execution_sender_normal;
            }
            else if let Ok(new_order) = meta.execution_reciever_low.try_recv() {
                order = Some(new_order);
                sender = &meta.execution_sender_low;
            } 

            YIELD_INFO.with(|info| {
                if let Some(yielder) = info.borrow_mut().yield_task.take() {
                    if order.is_some() {
                        yielder.wake();
                    } else {
                        order = Some(ExecutionOrder::ExecuteTask(yielder));
                    }
                }
            });
        }
    }
    meta.worker_count.fetch_sub(1, Ordering::Relaxed);
    for _ in 0..meta.worker_count.load(Ordering::Relaxed) {
        meta.signal_sender.send(RuntimeInfo::WakeUp);
    }
    println!("INFO:   Runtime worker ended.");
}

pub struct Runtime {
    pub(crate) meta: Arc<RuntimeMeta>,
    worker_joins: Mutex<Option<Vec<std::thread::JoinHandle<()>>>>,
}

impl Runtime {
    #[allow(unused)]
    pub fn new() -> Self {
        let (s_low, r_low) = crossbeam_channel::unbounded();
        let (s_normal, r_normal) = crossbeam_channel::unbounded();
        let (s_high, r_high) = crossbeam_channel::unbounded();
        let (s_very_high, r_very_high) = crossbeam_channel::unbounded();
        let (signal_snd, signal_rcv) = crossbeam_channel::unbounded();

        let worker_thread_count = usize::max(1,num_cpus::get_physical()-1);
        println!("INFO:   Runtime started with pool of {} threads.", worker_thread_count);

        let mut meta = Arc::new(RuntimeMeta{ 
            worker_count:                   AtomicU64::from(worker_thread_count as u64),
            execution_sender_low:           s_low,
            execution_sender_normal:        s_normal,
            execution_sender_high:          s_high,
            execution_sender_very_high:     s_very_high,
            execution_reciever_low:         r_low,
            execution_reciever_normal:      r_normal,
            execution_reciever_high:        r_high,
            execution_reciever_very_high:   r_very_high,
            signal_sender:                  signal_snd.clone(),
            signal_reciever:                signal_rcv,
            end_runtime:    	            std::sync::atomic::AtomicBool::from(false),
            open_tasks:                     std::sync::atomic::AtomicU64::from(0),
        });

        let worker_join_handles = (0..worker_thread_count)
            .into_iter()
            .map(|index|{
                let meta = meta.clone();
                std::thread::Builder::new()
                    .name(std::format!("worker thread {}", index))
                    .spawn(move || { 
                        // register thread/core to profiling
                        profiling::register_thread!(std::format!("worker thread {}", index).as_str());
                        worker(meta, index); 
                    })
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let ret = Self {
            meta: meta.clone(),
            worker_joins: Mutex::new(Some(worker_join_handles)),
        };

        ret
    }

    /**
     * Executes a prioritised future on a threadpool.
     * Submitted future should not block.
     * Submitted future should have a short runtime (<200mics) or yield periodicly.
     */
    #[allow(unused)]
    pub fn spawn_prioritised(&self, future: impl Future<Output = ()> + Send + Sync + 'static, priority: Priority) {
        let sender = match priority {
            Priority::Low => &self.meta.execution_sender_low,
            Priority::Normal => &self.meta.execution_sender_normal,
            Priority::High => &self.meta.execution_sender_high,
            Priority::VeryHigh => &self.meta.execution_sender_very_high,
        };

        let task_arc = Arc::new(Task{
            future: Mutex::new(Box::pin(future)),
            execution_sender: sender.clone(),
            priority: priority,
        });

        self.meta.open_tasks.fetch_add(1, Ordering::AcqRel);

        sender.send(ExecutionOrder::ExecuteTask(task_arc)).unwrap();
    }

    /**
     * Executes a future on a threadpool.
     * Submitted future should not block.
     * Submitted future should have a short runtime (<200mics) or yield periodicly.
     */
    #[allow(unused)]
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + Sync + 'static) {
        self.spawn_prioritised(future, Priority::Normal);
    }

    /**
     * Executes a prioritised closure on a threadpool.
     * Submitted Closures should not block.
     * Submitted Closures should have a short runtime (<200mics).
     * If the task needs to sync, please spawn a sync task via the spawn function.
     */
    #[allow(unused)]
    pub fn exec_prioritised(&self, closure: impl FnOnce() + Send + 'static, priority: Priority) {
        match priority {
            Priority::Low => self.meta.execution_sender_low.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
            Priority::Normal => self.meta.execution_sender_normal.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
            Priority::High => self.meta.execution_sender_high.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
            Priority::VeryHigh => self.meta.execution_sender_very_high.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
        }
    }
    
    /**
     * Executes a closure on a threadpool.
     * Submitted Closures should not block.
     * Submitted Closures should have a short runtime (<200mics).
     * If the task needs to sync, please spawn a sync task via the spawn function.
     */
    #[allow(unused)]
    pub fn exec(&self, closure: impl FnOnce() + Send + 'static) {
        self.meta.execution_sender_normal.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap();
    }

    /**
     * Kills threadpool.
     * All worker threads will terminate AFTER all open tasks are completed.
     * Looping tasks MUST be notified/terminated before calling this function!
    */
    #[allow(unused)]
    pub fn stop(&self) {
        if let Some(mut worker_joins) = self.worker_joins.lock().unwrap().take() {
            self.meta.end_runtime.store(true, Ordering::Release);

            let worker_count = worker_joins.len();

            for _ in 0..self.meta.worker_count.load(Ordering::Relaxed) {
                self.meta.signal_sender.send(RuntimeInfo::WakeUp);
            }

            println!("threads: {}", worker_joins.len());
    
            while let Some(join_handle) = worker_joins.pop() {
                join_handle.join().unwrap();
            }
    
            println!("INFO:   Runtime shut down.");
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop();
    }
}