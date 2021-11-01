use std::{pin::Pin, sync::Arc};

use futures::{Future};
use winit::{event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoop}, platform::windows::EventLoopExtWindows, window::{Window, WindowBuilder}};

use crate::{entity::EntityComponentManager, runtime::{Runtime, block_on}};
pub trait User : Send + Sync {
    fn init(self: Arc<Self>, meta: Arc<AppMeta>);
    fn cleanup(self: Arc<Self>, meta: Arc<AppMeta>);
    fn vary_tick(self: Arc<Self>, meta: Arc<AppMeta>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>;
    fn fixed_tick(self: Arc<Self>, meta: Arc<AppMeta>)-> Pin<Box<dyn Future<Output=()> + Send + Sync>>;
}

pub struct AppMeta {
    pub end_program: std::sync::atomic::AtomicBool,
    pub runtime: Runtime,
    pub ecm: EntityComponentManager,
    pub window: Window,
}

#[allow(unused)]
pub struct Application {
    meta: Arc<AppMeta>,
    event_loop: Option<EventLoop<()>>,
    user: Arc<dyn User>
}

impl Application {
    pub fn new(user: impl User + 'static) -> Self {
        let event_loop = EventLoop::new_any_thread();
        let window = WindowBuilder::new().build(&event_loop).unwrap();
        Self{
            meta: Arc::new(AppMeta{
                end_program: std::sync::atomic::AtomicBool::new(false),
                runtime: Runtime::new(),
                ecm: EntityComponentManager::new(),
                window: window,
            }),
            event_loop: Some(event_loop),
            user: Arc::new(user),
        }
    }

    fn cleanup(&self) {
        self.user.clone().cleanup(self.meta.clone());
    }

    async fn vary_loop(meta: Arc<AppMeta>, user: Arc<dyn User>) {
        user.vary_tick(meta.clone()).await;
        
        let earlier = std::time::SystemTime::now();
        crate::runtime::sleep_for(&meta.runtime, std::time::Duration::from_micros(1000)).await;
        let time_taken = std::time::SystemTime::now().duration_since(earlier).unwrap();
        println!("vary time sleep: {} mics", time_taken.as_micros());
    }

    async fn fixed_loop(meta: Arc<AppMeta>, user: Arc<dyn User>) {
        while !meta.end_program.fetch_or(false, std::sync::atomic::Ordering::Relaxed) {

            user.clone().fixed_tick(meta.clone()).await;

            let earlier = std::time::SystemTime::now();
            crate::runtime::sleep_for(&meta.runtime, std::time::Duration::from_micros(10_000)).await;
            let time_taken = std::time::SystemTime::now().duration_since(earlier).unwrap();
            println!("fixed time sleep: {} mics", time_taken.as_micros());
        }
    }

    pub fn run(mut self) {
        self.user.clone().init(self.meta.clone());

        let event_loop = self.event_loop.take().unwrap();

        self.meta.runtime.spawn_prioritised(Self::fixed_loop(self.meta.clone(), self.user.clone()), crate::runtime::task::Priority::VeryHigh);

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