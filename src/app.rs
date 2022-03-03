mod ticks;
use std::thread::JoinHandle;
use async_std::sync::Mutex;
use ticks::*;
pub use ticks::FixedData;
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
    fn init(self: Arc<Self>, appdata: Arc<SharedAppData>);
    fn cleanup(self: Arc<Self>, appdata: Arc<SharedAppData>);
    fn vary_tick(self: Arc<Self>, appdata: Arc<SharedAppData>) -> Pin<Box<dyn Future<Output=()> + Send + Sync>>;
    fn fixed_tick(self: Arc<Self>, appdata: Arc<SharedAppData>, fixed_data: Arc<FixedData>)-> Pin<Box<dyn Future<Output=()> + Send + Sync>>;
}

//o------------ App Data ---------------o

pub struct SharedAppData {
    pub end_program: AtomicBool,
    pub runtime: Runtime,
    pub ecm: EntityComponentManager,
    pub window: Window,
    pub renderer: Renderer,
    pub(crate) min_vary_delta_time: AtomicU64,
    pub(crate) vary_delta_time: AtomicU64,
    pub(crate) fixed_delta_time: AtomicU64,
    pub(crate) input_state_varstep: Mutex<InputState>,
    pub(crate) input_state_fixedstep: Mutex<InputState>,
}

impl SharedAppData {
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

    pub fn get_prev_frame_delta_time_secs(&self) -> f32 {
        self.vary_delta_time.load(Ordering::Relaxed) as f32 * 0.00_000_000_1
    }

    pub fn end(&self) {
        self.end_program.store(true, Ordering::Relaxed);
    }
    
    pub async fn key_pressed_varstep(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_varstep.lock().await;
        input_state.key_states[key as usize]
    }

    pub async fn key_released_varstep(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_varstep.lock().await;
        !input_state.key_states[key as usize]
    }

    pub async fn key_just_pressed_varstep(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_varstep.lock().await;
        input_state.key_states[key as usize] && !input_state.key_states_old[key as usize]
    }

    pub async fn key_just_released_varstep(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_varstep.lock().await;
        !input_state.key_states[key as usize] && input_state.key_states_old[key as usize]
    }

    pub async fn key_pressed_fixedstep(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_fixedstep.lock().await;
        input_state.key_states[key as usize]
    }

    pub async fn key_released_fixedstep(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_fixedstep.lock().await;
        !input_state.key_states[key as usize]
    }

    pub async fn key_just_pressed_fixedstep(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_fixedstep.lock().await;
        input_state.key_states[key as usize] && !input_state.key_states_old[key as usize]
    }

    pub async fn key_just_released_fixedstep(&self, key: VirtualKeyCode) -> bool {
        let input_state = &mut*self.input_state_fixedstep.lock().await;
        !input_state.key_states[key as usize] && input_state.key_states_old[key as usize]
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
                ecm: EntityComponentManager::new(),
                window,
                min_vary_delta_time: AtomicU64::from(100_000_000),
                vary_delta_time: AtomicU64::from(0),
                fixed_delta_time: AtomicU64::from(1_000_000),
                renderer,
                input_state_varstep: Mutex::new(InputState::default()),
                input_state_fixedstep: Mutex::new(InputState::default()),
            }),
            event_loop: Some(event_loop),
            user: Arc::new(T::default()),
            fixed_step_signal_thread: None,
            fixed_step_signal: async_std::channel::bounded(2),
            last_frame_end: Instant::now(),
        }
    }

    pub fn run(mut self) 
    {
        profiling::register_thread!("main thread");
        self.user.clone().init(self.shared_data.clone());

        let event_loop = self.event_loop.take().unwrap();

        self.shared_data.runtime.spawn_prioritised(fixed_loop(self.fixed_step_signal.1.clone(), self.shared_data.clone(), self.user.clone()), crate::sync::task::Priority::VeryHigh);

        let meta_clone = self.shared_data.clone();
        let signal_snd = self.fixed_step_signal.0.clone();
        self.fixed_step_signal_thread = Some(
            std::thread::Builder::new()
                .name("fixed time step notify thread".into())
                .spawn(||{fixed_time_step_notify(meta_clone, signal_snd)})
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
                        WindowEvent::Resized(physical_size)                            => block_on(self.shared_data.renderer.resize(*physical_size)),
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. }     => block_on(self.shared_data.renderer.resize(**new_inner_size)),
                        WindowEvent::KeyboardInput{device_id,input,is_synthetic} => {
                            let index = input.virtual_keycode.unwrap() as usize;
                            let in_state = &mut*spin_on!(self.shared_data.input_state_varstep.try_lock());

                            in_state.key_states_old[index] = in_state.key_states[index];
                            in_state.key_states[index] = match input.state { 
                                winit::event::ElementState::Pressed => true, 
                                winit::event::ElementState::Released => false 
                            };
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
            let vary_future = vary_tick(self.shared_data.clone(), self.user.clone());
            let vary_future = async move {
                let _d = dep;
                vary_future.await;
            };

            self.shared_data.runtime.spawn_prioritised(vary_future, crate::sync::task::Priority::VeryHigh);

            let clamped_time_taken = self.last_frame_end.elapsed().as_nanos().clamp(0, self.shared_data.get_min_delta_time_nanos() as u128) as u64;
            let left_time = self.shared_data.get_min_delta_time_nanos() - clamped_time_taken;

            spin_sleep::sleep(Duration::from_nanos(left_time));

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