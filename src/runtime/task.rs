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
    pub execution_sender: crossbeam_channel::Sender<ExecutionOrder>,
    pub priority: Priority,
}

pub enum ExecutionOrder {
    ExecuteTask(Arc<Task>),
    ExecuteClosure(ClosureBox),
    Die,
}

impl ArcWake for Task {
    fn wake(self: Arc<Self>) {
        self.execution_sender.send(ExecutionOrder::ExecuteTask(self.clone())).unwrap();
    }

    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.execution_sender.send(ExecutionOrder::ExecuteTask(arc_self.clone())).unwrap();
    }
}