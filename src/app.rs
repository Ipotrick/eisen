use std::{pin::Pin, sync::Arc};

use futures::{Future};
use winit::{event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoop}, platform::windows::EventLoopExtWindows, window::{Window, WindowBuilder}};

use crate::{entity::EntityComponentManager, sync::{Runtime, block_on}};
pub trait User : Send + Sync {
    fn init(self: Arc<Self>, appdata: Arc<AppData>);
    fn cleanup(self: Arc<Self>, appdata: Arc<AppData>);
    fn vary_tick(self: Arc<Self>, appdata: Arc<AppData>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>;
    fn fixed_tick(self: Arc<Self>, appdata: Arc<AppData>, fixed_data: Arc<FixedMeta>)-> Pin<Box<dyn Future<Output=()> + Send + Sync>>;
}

pub struct AppData {
    pub end_program: std::sync::atomic::AtomicBool,
    pub runtime: Runtime,
    pub ecm: EntityComponentManager,
    pub window: Window,
    fixed_delta_time: std::sync::atomic::AtomicU64,
}

impl AppData {
    pub fn get_fixed_delta_time_nanos(&self) -> u64 {
        self.fixed_delta_time.fetch_add(0, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_fixed_delta_time_secs(&self) -> f32 {
        self.fixed_delta_time.fetch_add(0, std::sync::atomic::Ordering::Relaxed) as f32 * 0.00_000_000
    }

    pub fn update_fixed_delta_time(&self, val: u64) {
        self.fixed_delta_time.store(val, std::sync::atomic::Ordering::Relaxed);
    }
}

#[allow(unused)]
pub struct Application {
    meta: Arc<AppData>,
    event_loop: Option<EventLoop<()>>,
    user: Arc<dyn User>,
}

#[allow(unused)]
pub struct FixedMeta {
    pub fixed_delta_time_nanos: u64,
    pub fixed_delta_time_secs: f32,
}

impl Application {
    pub fn new(user: impl User + 'static) -> Self {
        let event_loop = EventLoop::new_any_thread();
        let window = WindowBuilder::new().build(&event_loop).unwrap();
        Self{
            meta: Arc::new(AppData{
                end_program: std::sync::atomic::AtomicBool::new(false),
                runtime: Runtime::new(),
                ecm: EntityComponentManager::new(),
                window,
                fixed_delta_time: std::sync::atomic::AtomicU64::from(16_666_666),
            }),
            event_loop: Some(event_loop),
            user: Arc::new(user),
        }
    }

    fn cleanup(&self) {
        self.user.clone().cleanup(self.meta.clone());
    }

    async fn vary_loop(meta: Arc<AppData>, user: Arc<dyn User>) {
        user.vary_tick(meta.clone()).await;
        
        let earlier = std::time::SystemTime::now();
        let time_taken = std::time::SystemTime::now().duration_since(earlier).unwrap();
        println!("vary time taken: {} nanos", time_taken.as_nanos());
    }

    async fn fixed_loop(appdata: Arc<AppData>, user: Arc<dyn User>) {
        while !appdata.end_program.fetch_or(false, std::sync::atomic::Ordering::Relaxed) {
            let earlier = std::time::Instant::now();
            let fixed_data = Arc::new(FixedMeta{
                fixed_delta_time_nanos: appdata.get_fixed_delta_time_nanos(),
                fixed_delta_time_secs: appdata.get_fixed_delta_time_secs(),
            });

            user.clone().fixed_tick(appdata.clone(), fixed_data.clone()).await;

            crate::sync::sleep_until(&appdata.runtime, earlier + std::time::Duration::from_nanos(fixed_data.fixed_delta_time_nanos)).await;
        }
    }

    pub fn run(mut self) {
        self.user.clone().init(self.meta.clone());

        let event_loop = self.event_loop.take().unwrap();

        self.meta.runtime.spawn_prioritised(Self::fixed_loop(self.meta.clone(), self.user.clone()), crate::sync::task::Priority::VeryHigh);

        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::MainEventsCleared => {
                    self.meta.window.request_redraw();
                    if self.meta.end_program.fetch_or(false, std::sync::atomic::Ordering::Relaxed) {
                        self.cleanup();
                        *control_flow = ControlFlow::Exit
                    } else {
                        block_on(Self::vary_loop(self.meta.clone(), self.user.clone()));
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
                            self.cleanup();
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