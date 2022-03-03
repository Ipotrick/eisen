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

pub enum RuntimeInfo {
    WakeUp
}

type TaskFutureBox = Pin<Box<dyn Future<Output = ()> + Send + Sync>>;
type ClosureBox = Box<dyn FnOnce() + Send>;

#[derive(Clone,Copy)]
pub enum Priority {
    #[allow(unused)]
    VeryHigh,
    #[allow(unused)]
    High,
    #[allow(unused)]
    Normal,
    #[allow(unused)]
    Low
}

pub struct Task {
    pub future: Mutex<TaskFutureBox>,
    pub execution_queue: crossbeam_channel::Sender<ExecutionOrder>,
    pub notify_signal: crossbeam_channel::Sender<RuntimeInfo>,
    pub priority: Priority,
}

pub enum ExecutionOrder {
    ExecuteTask(Arc<Task>),
    ExecuteClosure(ClosureBox),
}

impl ArcWake for Task {
    fn wake(self: Arc<Self>) {
        if self.execution_queue.send(ExecutionOrder::ExecuteTask(self.clone())).is_err() {
            println!("WARNING: tried to wake up future on dead runtime!");
        }
        self.notify_signal.send(RuntimeInfo::WakeUp);
    }

    fn wake_by_ref(arc_self: &Arc<Self>) {
        if arc_self.execution_queue.send(ExecutionOrder::ExecuteTask(arc_self.clone())).is_err() {
            println!("WARNING: tried to wake up future on dead runtime!");
        }
        arc_self.notify_signal.send(RuntimeInfo::WakeUp);
    }
}