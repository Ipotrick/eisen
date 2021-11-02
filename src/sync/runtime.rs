pub use std::{cell::RefCell, pin::Pin, task::Waker};
pub use futures::{Future, FutureExt, task::{waker_ref}};
pub use smallbox::SmallBox;

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
use super::sleep::*;

pub(crate) struct RuntimeMeta {
    execution_sender_low: crossbeam_channel::Sender<ExecutionOrder>,
    execution_sender_normal: crossbeam_channel::Sender<ExecutionOrder>,
    execution_sender_high: crossbeam_channel::Sender<ExecutionOrder>,
    execution_sender_very_high: crossbeam_channel::Sender<ExecutionOrder>,
    execution_reciever_low: crossbeam_channel::Receiver<ExecutionOrder>,
    execution_reciever_normal: crossbeam_channel::Receiver<ExecutionOrder>,
    execution_reciever_high: crossbeam_channel::Receiver<ExecutionOrder>,
    execution_reciever_very_high: crossbeam_channel::Receiver<ExecutionOrder>,
    // sorted in reverse order of wake up time. Last Element will wake up first
    pub(crate) new_sleepers_snd: async_std::channel::Sender<SleepingTask>,
    end_program: std::sync::atomic::AtomicBool,
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
        finished
    };
}

#[allow(unused)]
fn worker(meta: Arc<RuntimeMeta>, _worker_index: usize) {
    'outer: loop {
        // if there are no tasks directly available, we sleep and wake up to execute the next available Order from any queue
        let (mut sender, mut order) = crossbeam_channel::select! {
            recv(meta.execution_reciever_low) -> order => (&meta.execution_sender_low, order.unwrap()),
            recv(meta.execution_reciever_normal) -> order => (&meta.execution_sender_normal, order.unwrap()),
            recv(meta.execution_reciever_high) -> order => (&meta.execution_sender_high, order.unwrap()),
            recv(meta.execution_reciever_very_high) -> order => (&meta.execution_sender_very_high, order.unwrap()),
        };

        'inner: loop {
            match order {
                ExecutionOrder::ExecuteTask(task) => {
                    process_task(&meta, sender, task);
                },
                ExecutionOrder::ExecuteClosure(mut closure) => {
                    closure();
                },
                ExecutionOrder::Die => break 'outer,
            };

            // after we executed an order, we try to directly execute other tasks but we also prioritise the orders from the channels
            if let Ok(new_order) = meta.execution_reciever_very_high.try_recv() {
                order = new_order;
                sender = &meta.execution_sender_very_high;
            }
            else if let Ok(new_order) = meta.execution_reciever_high.try_recv() {
                order = new_order;
                sender = &meta.execution_sender_high;
            }
            else if let Ok(new_order) = meta.execution_reciever_normal.try_recv() {
                order = new_order;
                sender = &meta.execution_sender_normal;
            }
            else if let Ok(new_order) = meta.execution_reciever_low.try_recv() {
                order = new_order;
                sender = &meta.execution_sender_low;
            }
            else {
                break 'inner;
            }
        }
    }
}

pub struct Runtime {
    pub(crate) meta: Arc<RuntimeMeta>,
    worker_joins: Vec<std::thread::JoinHandle<()>>,
}

impl Runtime {
    #[allow(unused)]
    pub fn new() -> Self {
        let (s_low, r_low) = crossbeam_channel::unbounded();
        let (s_normal, r_normal) = crossbeam_channel::unbounded();
        let (s_high, r_high) = crossbeam_channel::unbounded();
        let (s_very_high, r_very_high) = crossbeam_channel::unbounded();
        let (sleepers_snd, sleepers_rcv) = async_std::channel::unbounded();

        let mut meta = Arc::new(RuntimeMeta{ 
            execution_sender_low:           s_low,
            execution_sender_normal:        s_normal,
            execution_sender_high:          s_high,
            execution_sender_very_high:     s_very_high,
            execution_reciever_low:         r_low,
            execution_reciever_normal:      r_normal,
            execution_reciever_high:        r_high,
            execution_reciever_very_high:   r_very_high,
            new_sleepers_snd:               sleepers_snd,
            end_program:    	            std::sync::atomic::AtomicBool::from(false),
        });

        let worker_thread_count = usize::max(1,num_cpus::get_physical() - 1);
        println!("{} Threads are spawned for the Runtime.", worker_thread_count);
        let mut worker_join_handles = Vec::new();
        worker_join_handles.reserve(worker_thread_count);

        for index in 0..worker_thread_count {
            let idx = index;
            let meta = meta.clone();
            worker_join_handles.push(std::thread::spawn(move || { worker(meta, idx); }));
        }

        let ret = Self {
            meta: meta,
            worker_joins: worker_join_handles,
        };

        ret.spawn_prioritised(sleep_sheduler(sleepers_rcv), Priority::Low);

        ret
    }

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

        sender.send(ExecutionOrder::ExecuteTask(task_arc)).unwrap();
    }

    #[allow(unused)]
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + Sync + 'static) {
        self.spawn_prioritised(future, Priority::Normal);
    }

    #[allow(unused)]
    pub fn exec_prioritised(&self, closure: impl FnOnce() + Send + 'static, priority: Priority) {
        match priority {
            Priority::Low => self.meta.execution_sender_low.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
            Priority::Normal => self.meta.execution_sender_normal.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
            Priority::High => self.meta.execution_sender_high.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
            Priority::VeryHigh => self.meta.execution_sender_very_high.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
        }
    }
    
    #[allow(unused)]
    pub fn exec(&self, closure: impl FnOnce() + Send + 'static) {
        self.meta.execution_sender_normal.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap();
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.meta.end_program.fetch_or(true, std::sync::atomic::Ordering::Relaxed);

        for _ in 0..self.worker_joins.len() {
            self.meta.execution_sender_normal.send(ExecutionOrder::Die).unwrap();
        }

        while let Some(joinhandle) = self.worker_joins.pop() {
            let _ = joinhandle.join();
        }
    }
}