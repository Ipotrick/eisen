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


type TaskFutureBox = Pin<SmallBox<dyn Future<Output = ()> + Send + Sync, smallbox::space::S8>>;
type ClosureBox = SmallBox<dyn Fn() + Send, smallbox::space::S8>;

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
    pub future: TaskFutureBox,
    pub execution_sender: crossbeam_channel::Sender<ExecutionOrder>,
    pub priority: Priority,
}

pub struct TaskWrapper {
    pub task: Mutex<Option<Task>>,
}

pub enum ExecutionOrder {
    ExecuteTask(Arc<TaskWrapper>),
    ExecuteClosure(ClosureBox),
    Die,
}

impl ArcWake for TaskWrapper {
    fn wake(self: Arc<Self>) {
        self.task.lock().unwrap().as_ref().unwrap().execution_sender.send(ExecutionOrder::ExecuteTask(self.clone())).unwrap();
    }

    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.task.lock().unwrap().as_ref().unwrap().execution_sender.send(ExecutionOrder::ExecuteTask(arc_self.clone())).unwrap();
    }
}