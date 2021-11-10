use std::{sync::Arc};
use std::sync::atomic::*;

use super::*;

#[allow(unused)]
pub struct FixedData {
    pub fixed_delta_time_nanos: u64,
    pub fixed_delta_time_secs: f32,
}

pub(crate) async fn vary_tick(appdata: Arc<AppData>, user: Arc<dyn User>) {
    {
        profiling::scope!("vary_tick before user");

    }

    user.vary_tick(appdata.clone()).await;
}

async fn fixed_tick(appdata: Arc<AppData>, user: Arc<dyn User>) {
    let fixed_data = Arc::new(FixedData{
        fixed_delta_time_nanos: appdata.get_fixed_delta_time_nanos(),
        fixed_delta_time_secs: appdata.get_fixed_delta_time_secs(),
    });

    {
        profiling::scope!("fixed_tick before user","");

    };

    user.clone().fixed_tick(appdata.clone(), fixed_data.clone()).await;
}

pub(crate) async fn fixed_loop(signal: async_std::channel::Receiver<FixedStepUpdateSignal>, appdata: Arc<AppData>, user: Arc<dyn User>) {
    loop {
        let _ = signal.recv().await;
        if appdata.end_program.load(Ordering::Relaxed) {
            break;
        }
        fixed_tick(appdata.clone(), user.clone()).await;
    }
    println!("INFO:   ended fixed loop");
}