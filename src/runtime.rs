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

pub mod task;
use task::Task;
use task::ExecutionOrder;
use task::TaskWrapper;

struct BlockedThreadWaker{
    snd: crossbeam_channel::Sender<bool>,
}

impl ArcWake for BlockedThreadWaker {
    fn wake(self: Arc<Self>) {
        self.snd.send(true).unwrap();
    }

    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.snd.send(true).unwrap();
    }
}

struct RuntimeMeta {
    execution_sender_low: crossbeam_channel::Sender<ExecutionOrder>,
    execution_sender_normal: crossbeam_channel::Sender<ExecutionOrder>,
    execution_sender_high: crossbeam_channel::Sender<ExecutionOrder>,
    execution_sender_very_high: crossbeam_channel::Sender<ExecutionOrder>,
    execution_reciever_low: crossbeam_channel::Receiver<ExecutionOrder>,
    execution_reciever_normal: crossbeam_channel::Receiver<ExecutionOrder>,
    execution_reciever_high: crossbeam_channel::Receiver<ExecutionOrder>,
    execution_reciever_very_high: crossbeam_channel::Receiver<ExecutionOrder>,
    task_recycling_sender: crossbeam_channel::Sender<Arc<TaskWrapper>>,
    task_recycling_reciever: crossbeam_channel::Receiver<Arc<TaskWrapper>>,
    // sorted in reverse order of wake up time. Last Element will wake up first
    sleepers: Mutex<Vec<SleepingTask>>,
}

#[allow(unused)]
fn process_task(meta: &Arc<RuntimeMeta>, execution_sender: &crossbeam_channel::Sender<ExecutionOrder>, recycling_sender: &crossbeam_channel::Sender<Arc<TaskWrapper>>, task_wrapper: Arc<TaskWrapper>) {
    let task_finished = {
        let waker = waker_ref(&task_wrapper);
        let context = &mut Context::from_waker(&*waker);

        CURRENT_RUNTIME_META.with(|f|{
            *f.borrow_mut() = Some(meta.clone());
        });
    
        let finished = Poll::Pending != task_wrapper.task.lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .future.as_mut()
            .poll(context);

            CURRENT_RUNTIME_META.with(|f|{
            *f.borrow_mut() = None;
        });

        finished
    };

    if task_finished {
        *task_wrapper.task.lock().unwrap() = None;
        if Arc::strong_count(&task_wrapper) == 1 /* we can only recycle it if we have the last reference to the wrapper */ {
            recycling_sender.send(task_wrapper).unwrap();
        }
    } else if Arc::strong_count(&task_wrapper) == 1 {
        // if the task is not finished and there is no references to wake the task later, we just requeue the task immediately
        execution_sender.send(ExecutionOrder::ExecuteTask(task_wrapper)).unwrap();
    }
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
            default(std::time::Duration::from_millis(10)) => {
                if let Ok(mut sleepers) = meta.sleepers.try_lock() {
                    while !sleepers.is_empty() && sleepers.last().unwrap().wake_up_time <= std::time::Instant::now() {
                        let sleeper = sleepers.pop().unwrap();
                        sleeper.waker.wake_by_ref();
                    }
                }
                continue 'outer;
            }
        };

        'inner: loop {
            match order {
                ExecutionOrder::ExecuteTask(task) => { 
                    process_task(&meta, sender, &meta.task_recycling_sender, task);
                },
                ExecutionOrder::ExecuteClosure(closure) => {
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
    meta: Arc<RuntimeMeta>,
    worker_joins: Vec<std::thread::JoinHandle<()>>,
}

thread_local! {
    static CURRENT_RUNTIME_META: RefCell<Option<Arc<RuntimeMeta>>> = RefCell::new(None);
}
struct SleepFuture {
    wake_up_time: std::time::Instant,
}

struct SleepingTask {
    waker: Waker,
    wake_up_time: std::time::Instant,
}

impl Future for SleepFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.wake_up_time < std::time::Instant::now() {
            Poll::Ready(())
        } else {
            CURRENT_RUNTIME_META.with(|f|{
                let mut gf = f
                .borrow_mut();
                let mut g = gf
                .as_mut()
                .unwrap().sleepers
                .lock()
                .unwrap();

                // insert, so that the shorter waittimes are at the end of the vec
                let insertion_index = {
                    let mut index = g.len();
                    loop {
                        if index == 0 || g[index-1].wake_up_time > self.wake_up_time {
                            break;
                        }
                        index -= 1;
                    }
                    index
                };

                g.insert(insertion_index, SleepingTask{waker: cx.waker().clone(), wake_up_time: self.wake_up_time});
            });
            Poll::Pending
        }
    }
}

#[allow(unused)]
pub async fn sleep_for_task(min_dura: std::time::Duration) {
    let sleeper = SleepFuture{wake_up_time: std::time::Instant::now() + min_dura};
    sleeper.await;

}

impl Runtime {
    #[allow(unused)]
    pub fn new() -> Self {
        let (s_low, r_low) = crossbeam_channel::unbounded();
        let (s_normal, r_normal) = crossbeam_channel::unbounded();
        let (s_high, r_high) = crossbeam_channel::unbounded();
        let (s_very_high, r_very_high) = crossbeam_channel::unbounded();
        let (recycle_s, recycle_r) = crossbeam_channel::unbounded();

        let meta = Arc::new(RuntimeMeta{ 
            execution_sender_low:           s_low,
            execution_sender_normal:        s_normal,
            execution_sender_high:          s_high,
            execution_sender_very_high:     s_very_high,
            execution_reciever_low:         r_low,
            execution_reciever_normal:      r_normal,
            execution_reciever_high:        r_high,
            execution_reciever_very_high:   r_very_high,
            task_recycling_reciever:        recycle_r,
            task_recycling_sender:          recycle_s,
            sleepers: Mutex::new(Vec::new())
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

        Self {
            meta: meta,
            worker_joins: worker_join_handles,
        }
    }

    #[allow(unused)]
    fn make_task(&self, future: impl Future<Output = ()> + Send + Sync + 'static, priority: task::Priority, sender: &crossbeam_channel::Sender<ExecutionOrder>) -> Arc<TaskWrapper> {
        match self.meta.task_recycling_reciever.try_recv() {
            Ok(rcv) => {
                {
                    assert_eq!(Arc::strong_count(&rcv), 1);
                    let g = &mut *rcv.task.lock().unwrap();
                    assert!(g.is_none());
                    *g = Some(
                        Task{
                            execution_sender: sender.clone(),
                            future: unsafe{Pin::new_unchecked(smallbox::smallbox!(future))},
                            priority: priority,
                        }
                    );
                }
                rcv
            },
            Err(_) => Arc::new(TaskWrapper{
                task: Mutex::new(Some(Task{
                    future: unsafe{Pin::new_unchecked(smallbox::smallbox!(future))}, 
                    execution_sender: sender.clone(),
                    priority: priority,
                })) 
            })
        }
    }

    #[allow(unused)]
    pub fn spawn_prioritised(&self, future: impl Future<Output = ()> + Send + Sync + 'static, priority: task::Priority) {
        let task_arc = self.make_task(
            future, 
            priority, 
            match priority {
                task::Priority::Low => &self.meta.execution_sender_low,
                task::Priority::Normal => &self.meta.execution_sender_normal,
                task::Priority::High => &self.meta.execution_sender_high,
                task::Priority::VeryHigh => &self.meta.execution_sender_very_high,
            }
        );

        match priority {
            task::Priority::Low => self.meta.execution_sender_low.send(ExecutionOrder::ExecuteTask(task_arc)).unwrap(),
            task::Priority::Normal => self.meta.execution_sender_normal.send(ExecutionOrder::ExecuteTask(task_arc)).unwrap(),
            task::Priority::High => self.meta.execution_sender_high.send(ExecutionOrder::ExecuteTask(task_arc)).unwrap(),
            task::Priority::VeryHigh => self.meta.execution_sender_very_high.send(ExecutionOrder::ExecuteTask(task_arc)).unwrap(),
        }
    }

    #[allow(unused)]
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + Sync + 'static) {
        self.spawn_prioritised(future, task::Priority::Normal);
    }

    #[allow(unused)]
    pub fn exec_prioritised(&self, closure: impl Fn() + Send + 'static, priority: task::Priority) {
        match priority {
            task::Priority::Low => self.meta.execution_sender_low.send(ExecutionOrder::ExecuteClosure(smallbox::smallbox!(closure))).unwrap(),
            task::Priority::Normal => self.meta.execution_sender_normal.send(ExecutionOrder::ExecuteClosure(smallbox::smallbox!(closure))).unwrap(),
            task::Priority::High => self.meta.execution_sender_high.send(ExecutionOrder::ExecuteClosure(smallbox::smallbox!(closure))).unwrap(),
            task::Priority::VeryHigh => self.meta.execution_sender_very_high.send(ExecutionOrder::ExecuteClosure(smallbox::smallbox!(closure))).unwrap(),
        }
    }
    
    #[allow(unused)]
    pub fn exec(&self, closure: impl Fn() + Send + 'static) {
        self.meta.execution_sender_normal.send(ExecutionOrder::ExecuteClosure(smallbox::smallbox!(closure))).unwrap();
    }

    #[allow(unused)]
    pub fn block_on(&self, mut future: impl Future<Output = ()> + Send + Sync + 'static) {
        let futur_ref = &future;
        let (snd,rcv) = crossbeam_channel::unbounded::<bool>();
        snd.send( true ).unwrap();

        let waker = Arc::new(BlockedThreadWaker{snd:snd});
        let waker = waker_ref(&waker);
        let context = &mut Context::from_waker(&*waker);

        loop {
            let _ = rcv.recv().unwrap();
            let future = unsafe{ Pin::new_unchecked(&mut future)};
            if future.poll(context) != Poll::Pending {
                break;
            }
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        for _ in 0..self.worker_joins.len() {
            self.meta.execution_sender_normal.send(ExecutionOrder::Die).unwrap();
        }

        while let Some(joinhandle) = self.worker_joins.pop() {
            let _ = joinhandle.join();
        }
    }
}