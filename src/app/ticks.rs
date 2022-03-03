use std::{sync::Arc};
use std::sync::atomic::*;

use super::*;

#[allow(unused)]
pub struct FixedData {
    pub fixed_delta_time_nanos: u64,
    pub fixed_delta_time_secs: f32,
}

pub(crate) struct FixedStepUpdateSignal;

pub(crate) fn fixed_time_step_notify(meta: Arc<SharedAppData>, signal_snd: async_std::channel::Sender<FixedStepUpdateSignal>) {
    profiling::register_thread!("fixed time step notify thread".into());
    while !meta.end_program.load(Ordering::Relaxed) {
        spin_sleep::sleep(Duration::from_nanos(meta.get_fixed_delta_time_nanos()));
        profiling::scope!("fixed step notify");
        if signal_snd.len() < 2 {
            let _ = signal_snd.try_send(FixedStepUpdateSignal{});
        }
    }
}

pub(crate) async fn fixed_loop<T: User>(signal: async_std::channel::Receiver<FixedStepUpdateSignal>, appdata: Arc<SharedAppData>, user: Arc<T>) 
{
    loop {
        let _ = signal.recv().await;
        if appdata.end_program.load(Ordering::Relaxed) {
            break;
        }
        fixed_tick(appdata.clone(), user.clone()).await;
    }
    println!("INFO:   ended fixed loop");
}

async fn fixed_tick<T: User>(appdata: Arc<SharedAppData>, user: Arc<T>) {
    let fixed_data = Arc::new(FixedData{
        fixed_delta_time_nanos: appdata.get_fixed_delta_time_nanos(),
        fixed_delta_time_secs: appdata.get_fixed_delta_time_secs(),
    });

    {
        profiling::scope!("fixed_tick before user");
    }

    user.fixed_tick(appdata.clone(), fixed_data.clone()).await;
}

pub(crate) async fn vary_tick<T: User>(appdata: Arc<SharedAppData>, user: Arc<T>) {
    {
        profiling::scope!("vary_tick before user");
    }
    user.vary_tick(appdata.clone()).await;
    appdata.renderer.render().await.unwrap();
}