mod ticks;
use std::thread::JoinHandle;
use async_std::sync::Mutex;
use ticks::*;
use winit::event::VirtualKeyCode;

use std::time::{Duration, Instant};
use std::{pin::Pin, sync::Arc};
use std::sync::atomic::*;

use futures::{Future};
use winit::{event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoop}, platform::windows::EventLoopExtWindows, window::{Window, WindowBuilder}};

use crate::rendering::Renderer;
use crate::sync::AtomicWaiter;
use crate::{entity::EntityComponentManager, sync::{Runtime, block_on}};

//o------------ User Trait ---------------o

pub trait User : Send + Sync + Default
{
    fn init(self: Arc<Self>, shared_data: Arc<SharedAppData>, fixed_step_data: Arc<FixedStepData>, variable_step_data: Arc<VariableStepData>);
    fn cleanup(self: Arc<Self>, shared_data: Arc<SharedAppData>);
    fn fixed_step(self: Arc<Self>, shared_data: Arc<SharedAppData>, fixed_step_data: Arc<FixedStepData>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>;
    fn varaible_step(self: Arc<Self>, shared_data: Arc<SharedAppData>, variable_step_data: Arc<VariableStepData>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>;
}

//o------------ App Data ---------------o

pub struct SharedAppData {
    pub end_program: AtomicBool,
    pub runtime: Runtime,
    pub window: Window,
    pub(crate) min_vary_delta_time: AtomicU64,
}

pub struct FixedStepData {
    pub(crate) input_state: Mutex<InputState>,
    pub ecm: EntityComponentManager,
    pub(crate) fixed_delta_time: AtomicU64,
}

pub struct VariableStepData {
    pub(crate) input_state_backbuffer: Mutex<InputState>,
    pub(crate) input_state_frontbuffer: Mutex<InputState>,
    pub renderer: Renderer,
    pub(crate) vary_delta_time: AtomicU64,
}

impl SharedAppData {
    pub fn end(&self) {
        self.end_program.store(true, Ordering::Relaxed);
    }
}

impl FixedStepData {
    pub async fn key_pressed(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state.lock().await;
        input_state.key_states[key as usize]
    }

    pub async fn key_released(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state.lock().await;
        !input_state.key_states[key as usize]
    }

    pub async fn key_just_pressed(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state.lock().await;
        input_state.key_states[key as usize] && !input_state.key_states_old[key as usize]
    }

    pub async fn key_just_released(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state.lock().await;
        !input_state.key_states[key as usize] && input_state.key_states_old[key as usize]
    }

    pub fn get_delta_time(&self) -> std::time::Duration {
        let dt = self.fixed_delta_time.load(std::sync::atomic::Ordering::Relaxed);
        std::time::Duration::from_nanos(dt)
    }
}

impl VariableStepData {
    pub async fn key_pressed(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_backbuffer.lock().await;
        input_state.key_states[key as usize]
    }

    pub async fn key_released(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_backbuffer.lock().await;
        !input_state.key_states[key as usize]
    }

    pub async fn key_just_pressed(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_backbuffer.lock().await;
        input_state.key_states[key as usize] && !input_state.key_states_old[key as usize]
    }

    pub async fn key_just_released(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_backbuffer.lock().await;
        !input_state.key_states[key as usize] && input_state.key_states_old[key as usize]
    }

    pub fn get_delta_time(&self) -> std::time::Duration {
        let dt = self.vary_delta_time.load(std::sync::atomic::Ordering::Relaxed);
        std::time::Duration::from_nanos(dt)
    }
}

//o------------ Application ---------------o

pub(crate) struct InputState {
    pub(crate) key_states_old: Box<[bool; 512]>,
    pub(crate) key_states: Box<[bool; 512]>,
    pub(crate) button_states_old: Box<[bool; 16]>,
    pub(crate) button_states: Box<[bool; 16]>,
    pub(crate) cursor_pos_old: [i32; 2],
    pub(crate) cursor_pos: [i32; 2],
}

impl Default for InputState {
    fn default() -> Self {
        Self{
            key_states_old: Box::new([false; 512]),
            key_states: Box::new([false; 512]),
            button_states_old: Box::new([false; 16]),
            button_states: Box::new([false; 16]),
            cursor_pos_old: [0; 2],
            cursor_pos: [0; 2],
        }
    }
}

#[allow(unused)]
pub struct Application<T : User> 
{
    pub(crate) shared_data: Arc<SharedAppData>,
    pub(crate) fixed_step_data: Arc<FixedStepData>,
    pub(crate) variable_step_data: Arc<VariableStepData>,
    event_loop: Option<EventLoop<()>>,
    user: Arc<T>,
    fixed_step_signal_thread: Option<JoinHandle<()>>,
    fixed_step_signal: (async_std::channel::Sender<FixedStepUpdateSignal>, async_std::channel::Receiver<FixedStepUpdateSignal>),
    last_frame_end: Instant,
}

impl<T: User + 'static> Application<T> 
{

    pub fn new() -> Self 
    {
        env_logger::init();
        let event_loop = EventLoop::new_any_thread();
        let window = WindowBuilder::new().build(&event_loop).unwrap();
        let renderer = block_on(Renderer::new(&window));
        Self{
            shared_data: Arc::new(SharedAppData{
                end_program: AtomicBool::new(false),
                runtime: Runtime::new(),
                window,
                min_vary_delta_time: AtomicU64::from(1_000_000),
            }),
            fixed_step_data: Arc::new(FixedStepData{
                ecm: EntityComponentManager::new(),
                input_state: Mutex::new(InputState::default()),
                fixed_delta_time: AtomicU64::from(33_000_000),
            }),
            event_loop: Some(event_loop),
            user: Arc::new(T::default()),
            fixed_step_signal_thread: None,
            fixed_step_signal: async_std::channel::bounded(2),
            last_frame_end: Instant::now(),
            variable_step_data: Arc::new(VariableStepData{
                input_state_backbuffer: Mutex::new(InputState::default()),
                input_state_frontbuffer: Mutex::new(InputState::default()),
                renderer,
                vary_delta_time: AtomicU64::from(0),
            }),
        }
    }

    pub fn run(mut self) 
    {
        profiling::register_thread!("main thread");
        self.user.clone().init(self.shared_data.clone(), self.fixed_step_data.clone(), self.variable_step_data.clone());

        let event_loop = self.event_loop.take().unwrap();

        self.shared_data.runtime.spawn_prioritised(fixed_loop(
            self.fixed_step_signal.1.clone(), 
            self.shared_data.clone(), 
            self.fixed_step_data.clone(),
            self.variable_step_data.clone(),
            self.user.clone()
        ), crate::sync::task::Priority::VeryHigh);

        let meta_clone = self.shared_data.clone();
        let fixed_step_data_clone = self.fixed_step_data.clone();
        let signal_snd = self.fixed_step_signal.0.clone();
        self.fixed_step_signal_thread = Some(
            std::thread::Builder::new()
                .name("fixed time step notify thread".into())
                .spawn(||{fixed_time_step_notify(meta_clone, fixed_step_data_clone, signal_snd)})
                .unwrap()
        ); 

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                Event::MainEventsCleared => self.on_main_events_cleared(control_flow),
                Event::RedrawRequested(_) => { },
                Event::WindowEvent{ ref event,  window_id, } if (window_id == self.shared_data.window.id()) => {
                    match event {
                        WindowEvent::CloseRequested                                                     => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size)                            => block_on(self.variable_step_data.renderer.resize(*physical_size)),
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. }     => block_on(self.variable_step_data.renderer.resize(**new_inner_size)),
                        WindowEvent::KeyboardInput{device_id,input,is_synthetic} => {
                            let index = input.virtual_keycode.unwrap() as usize;
                            let in_state = &mut*spin_on!(self.variable_step_data.input_state_backbuffer.try_lock());

                            in_state.key_states[index] = input.state == winit::event::ElementState::Pressed;
                        },
                        _ => { }
                    }
                },
                _ => {}
            }
        });
    }

    fn on_main_events_cleared(&mut self, control_flow: &mut ControlFlow) 
    {
        profiling::scope!("MainEventsCleared");
        self.shared_data.window.request_redraw();
        if self.shared_data.end_program.load(Ordering::Relaxed) {
            *control_flow = ControlFlow::Exit
        } else {
            let waiter = AtomicWaiter::new();
            let dep = waiter.make_dependency();
            let vary_future = vary_tick(self.shared_data.clone(), self.variable_step_data.clone(), self.user.clone());
            let vary_future = async move {
                let _d = dep;
                vary_future.await;
            };

            self.shared_data.runtime.spawn_prioritised(vary_future, crate::sync::task::Priority::VeryHigh);

            let clamped_time_taken = self.last_frame_end.elapsed().clamp(std::time::Duration::from_micros(0), self.variable_step_data.get_delta_time());
            let left_time = self.variable_step_data.get_delta_time() - clamped_time_taken;

            spin_sleep::sleep(left_time);

            block_on(waiter);
            profiling::finish_frame!();
            self.last_frame_end = Instant::now();
        }
    }
}

impl<T: User> Drop for Application<T>
{
    fn drop(&mut self) 
    {
        self.user.clone().cleanup(self.shared_data.clone());
        self.shared_data.end_program.store(true, Ordering::Relaxed);
        let _ = self.fixed_step_signal.0.try_send(FixedStepUpdateSignal{});
        self.shared_data.runtime.stop();
        if let Some(t) = self.fixed_step_signal_thread.take() {
            t.join().unwrap();
        }
    }
}