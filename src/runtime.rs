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
    // sorted in reverse order of wake up time. Last Element will wake up first
    new_sleepers_snd: async_std::channel::Sender<SleepingTask>,
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
    meta: Arc<RuntimeMeta>,
    worker_joins: Vec<std::thread::JoinHandle<()>>,
}

const SLEEP_SPIN_TIME: std::time::Duration = std::time::Duration::from_micros(100);

struct SleepFuture {
    called_once: bool,
    wake_up_time: std::time::Instant,
    sender: async_std::channel::Sender<SleepingTask>,
}

struct SleepingTask {
    waker: Waker,
    wake_up_time: std::time::Instant,
}

impl Future for SleepFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.called_once && std::time::Instant::now() + SLEEP_SPIN_TIME > self.wake_up_time {
            while std::time::Instant::now() < self.wake_up_time {}
            return Poll::Ready(());
        }
        self.called_once = true;
        while self.sender.try_send(SleepingTask{waker: cx.waker().clone(), wake_up_time: self.wake_up_time}).is_err() {}
        Poll::Pending
    }
}

#[allow(unused)]
pub async fn sleep_for(runtime: &Runtime, min_dura: std::time::Duration) {
    let wake_up_time = std::time::Instant::now() + min_dura;
    let sleeper = SleepFuture{
        called_once: false,
        wake_up_time: wake_up_time,
        sender: runtime.meta.new_sleepers_snd.clone(),
    };

    sleeper.await;
}

struct YieldFuture {
    yielded_once: bool,
}

impl Future for YieldFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded_once {
            return Poll::Ready(());
        } 
        self.yielded_once = true;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

#[allow(unused)]
pub async fn yield_now() {
    YieldFuture{yielded_once:false}.await;
}

async fn sleep_sheduler(sleepers_rcv: async_std::channel::Receiver<SleepingTask>) {
    let mut sleepers = Vec::new();
    loop {
        if sleepers.is_empty() {
            let sleeper = sleepers_rcv.recv().await.unwrap();
            sleepers.push(sleeper);
        }

        while let Ok(new_sleeper) = sleepers_rcv.try_recv() {
            let insertion_index = sleepers.partition_point(|other_st: &SleepingTask| other_st.wake_up_time > new_sleeper.wake_up_time);
            sleepers.insert(insertion_index, new_sleeper);
        }

        'search: loop {
            if sleepers.is_empty() { break 'search; }

            if (std::time::Instant::now() + SLEEP_SPIN_TIME) > sleepers.last().unwrap().wake_up_time {
                let sleeper = sleepers.pop().unwrap();
                sleeper.waker.wake_by_ref();
            } else {
                break 'search;
            }
        }

        yield_now().await;
    }
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

        ret.spawn_prioritised(sleep_sheduler(sleepers_rcv), task::Priority::Low);

        ret
    }

    #[allow(unused)]
    pub fn spawn_prioritised(&self, future: impl Future<Output = ()> + Send + Sync + 'static, priority: task::Priority) {
        let sender = match priority {
            task::Priority::Low => &self.meta.execution_sender_low,
            task::Priority::Normal => &self.meta.execution_sender_normal,
            task::Priority::High => &self.meta.execution_sender_high,
            task::Priority::VeryHigh => &self.meta.execution_sender_very_high,
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
        self.spawn_prioritised(future, task::Priority::Normal);
    }

    #[allow(unused)]
    pub fn exec_prioritised(&self, closure: impl FnOnce() + Send + 'static, priority: task::Priority) {
        match priority {
            task::Priority::Low => self.meta.execution_sender_low.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
            task::Priority::Normal => self.meta.execution_sender_normal.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
            task::Priority::High => self.meta.execution_sender_high.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
            task::Priority::VeryHigh => self.meta.execution_sender_very_high.send(ExecutionOrder::ExecuteClosure(Box::new(closure))).unwrap(),
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

#[allow(unused)]
pub fn block_on<Out>(mut future: impl Future<Output = Out>) -> Out {
    let (snd,rcv) = crossbeam_channel::unbounded::<bool>();
    snd.send( true ).unwrap();

    let waker = Arc::new(BlockedThreadWaker{snd:snd});
    let waker = waker_ref(&waker);
    let context = &mut Context::from_waker(&*waker);

    loop {
        let _ = rcv.recv().unwrap();
        let future = unsafe{ Pin::new_unchecked(&mut future)};
        if let Poll::Ready(val) = future.poll(context) {
            break val;
        }
    }
}