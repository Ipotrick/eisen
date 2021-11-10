mod ticks;
use std::thread::JoinHandle;
use ticks::*;
pub use ticks::FixedData;

use std::time::{Duration, Instant};
use std::{pin::Pin, sync::Arc};
use std::sync::atomic::*;

use async_std::sync::Mutex;
use futures::{Future};
use winit::{event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoop}, platform::windows::EventLoopExtWindows, window::{Window, WindowBuilder}};

use crate::sync::AtomicWaiter;
use crate::{entity::EntityComponentManager, sync::{Runtime, block_on}};
pub trait User : Send + Sync {
    fn init(self: Arc<Self>, appdata: Arc<AppData>);
    fn cleanup(self: Arc<Self>, appdata: Arc<AppData>);
    fn vary_tick(self: Arc<Self>, appdata: Arc<AppData>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>;
    fn fixed_tick(self: Arc<Self>, appdata: Arc<AppData>, fixed_data: Arc<FixedData>)-> Pin<Box<dyn Future<Output=()> + Send + Sync>>;
}

pub struct AppData {
    pub end_program: AtomicBool,
    pub runtime: Runtime,
    pub ecm: EntityComponentManager,
    pub window: Window,
    pub(crate) min_vary_delta_time: AtomicU64,
    pub(crate) vary_delta_time: AtomicU64,
    pub(crate) fixed_delta_time: AtomicU64,
}

impl AppData {
    pub fn get_fixed_delta_time_nanos(&self) -> u64 {
        self.fixed_delta_time.load(Ordering::Relaxed)
    }

    pub fn get_fixed_delta_time_secs(&self) -> f32 {
        self.fixed_delta_time.load(Ordering::Relaxed) as f32 * 0.00_000_000_1
    }

    pub fn update_fixed_delta_time(&self, val: u64) {
        self.fixed_delta_time.store(val, Ordering::Relaxed);
    }

    pub fn get_min_delta_time_nanos(&self) -> u64 {
        self.min_vary_delta_time.load(Ordering::Relaxed)
    }

    pub fn get_min_delta_time_secs(&self) -> f32 {
        self.min_vary_delta_time.load(Ordering::Relaxed) as f32 * 0.00_000_000_1
    }

    pub fn update_min_delta_time(&self, val: u64) {
        self.min_vary_delta_time.store(val, Ordering::Relaxed);
    }
    
    pub fn get_prev_frame_delta_time_nanos(&self) -> u64 {
        self.vary_delta_time.load(Ordering::Relaxed)
    }

    pub fn get_rev_frame_delta_time_secs(&self) -> f32 {
        self.vary_delta_time.load(Ordering::Relaxed) as f32 * 0.00_000_000_1
    }

    pub fn end(&self) {
        self.end_program.store(true, Ordering::Relaxed);
    }
}

pub(crate) struct FixedStepUpdateSignal;

#[allow(unused)]
pub struct Application {
    meta: Arc<AppData>,
    event_loop: Option<EventLoop<()>>,
    user: Arc<dyn User>,
    fixed_step_signal_thread: Option<JoinHandle<()>>,
    fixed_step_signal: (async_std::channel::Sender<FixedStepUpdateSignal>, async_std::channel::Receiver<FixedStepUpdateSignal>),
}

impl Drop for Application {
    fn drop(&mut self) {
        self.user.clone().cleanup(self.meta.clone());
        self.meta.end_program.store(true, Ordering::Relaxed);
        let _ = self.fixed_step_signal.0.try_send(FixedStepUpdateSignal{});
        self.meta.runtime.stop();
        if let Some(t) = self.fixed_step_signal_thread.take() {
            t.join().unwrap();
        }
    }
}

impl Application {
    pub fn new(user: impl User + 'static) -> Self {
        let event_loop = EventLoop::new_any_thread();
        let window = WindowBuilder::new().build(&event_loop).unwrap();
        Self{
            meta: Arc::new(AppData{
                end_program: AtomicBool::new(false),
                runtime: Runtime::new(),
                ecm: EntityComponentManager::new(),
                window,
                min_vary_delta_time: AtomicU64::from(10_000_000),
                vary_delta_time: AtomicU64::from(0),
                fixed_delta_time: AtomicU64::from(16_666_666),
            }),
            event_loop: Some(event_loop),
            user: Arc::new(user),
            fixed_step_signal_thread: None,
            fixed_step_signal: async_std::channel::bounded(2),
        }
    }

    pub fn run(mut self) {
        profiling::register_thread!("main thread");
        self.user.clone().init(self.meta.clone());

        let event_loop = self.event_loop.take().unwrap();

        self.meta.runtime.spawn_prioritised(fixed_loop(self.fixed_step_signal.1.clone(), self.meta.clone(), self.user.clone()), crate::sync::task::Priority::VeryHigh);

        let meta_clone = self.meta.clone();
        let signal = self.fixed_step_signal.0.clone();
        self.fixed_step_signal_thread = Some(std::thread::spawn(move || {
            let meta = meta_clone;
            while !meta.end_program.load(Ordering::Relaxed) {
                spin_sleep::sleep(Duration::from_nanos(meta.get_fixed_delta_time_nanos()));
                if signal.len() < 2 {
                    let _ = signal.try_send(FixedStepUpdateSignal{});
                }
            }
        }));

        let mut last_frame_end = Instant::now();

        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::MainEventsCleared => {
                    profiling::scope!("main event");
                    self.meta.window.request_redraw();
                    if self.meta.end_program.load(Ordering::Relaxed) {
                        *control_flow = ControlFlow::Exit
                    } else {
                        let waiter = AtomicWaiter::new();
                        let dep = waiter.make_dependency();
                        let vary_future = vary_tick(self.meta.clone(), self.user.clone());
                        let vary_future = async move {
                            let _d = dep;
                            vary_future.await;
                        };

                        self.meta.runtime.spawn_prioritised(vary_future, crate::sync::task::Priority::VeryHigh);

                        let clamped_time_taken = last_frame_end.elapsed().as_nanos().clamp(0, self.meta.get_min_delta_time_nanos() as u128) as u64;
                        let left_time = self.meta.get_min_delta_time_nanos() - clamped_time_taken;

                        spin_sleep::sleep(Duration::from_nanos(left_time));

                        block_on(waiter);
                        profiling::finish_frame!();
                        last_frame_end = Instant::now();
                    }
                }
                Event::RedrawRequested(_) => {
                }
                Event::WindowEvent{
                    ref event,
                    window_id,
                } if (window_id == self.meta.window.id()) => {
                    match event {
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit
                        },
                        _ => { }
                    }
                },
                _ => {}
            }
        });
    }
}