use std::{sync::Arc};
use std::sync::atomic::*;

use super::*;

pub(crate) struct FixedStepUpdateSignal;

pub(crate) fn fixed_time_step_notify(
    shared_data: Arc<SharedAppData>, 
    fixed_step_data: Arc<FixedStepData>,
    signal_snd: async_std::channel::Sender<FixedStepUpdateSignal>
) {
    profiling::register_thread!("fixed time step notify thread".into());
    while !shared_data.end_program.load(Ordering::Relaxed) {
        spin_sleep::sleep(fixed_step_data.get_delta_time());
        profiling::scope!("fixed step notify");
        if signal_snd.len() < 2 {
            let _ = signal_snd.try_send(FixedStepUpdateSignal{});
        }
    }
}

pub(crate) async fn fixed_loop<T: User>(
    signal: async_std::channel::Receiver<FixedStepUpdateSignal>, 
    shared_data: Arc<SharedAppData>, 
    fixed_step_data: Arc<FixedStepData>, 
    variable_step_data: Arc<VariableStepData>, 
    user: Arc<T>
) {
    loop {
        let _ = signal.recv().await;
        if shared_data.end_program.load(Ordering::Relaxed) {
            break;
        }
        fixed_tick(shared_data.clone(), fixed_step_data.clone(), variable_step_data.clone(), user.clone()).await;
    }
    println!("INFO:   ended fixed loop");
}

async fn fixed_tick<T: User>(shared_data: Arc<SharedAppData>, fixed_step_data: Arc<FixedStepData>, variable_step_data: Arc<VariableStepData>, user: Arc<T>) {
    {
        let input_state_varstep = &mut*variable_step_data.input_state_frontbuffer.lock().await;
        let input_state_fixedstep = &mut*fixed_step_data.input_state.lock().await;

        for i in 0..512 {
            input_state_fixedstep.key_states_old[i] = input_state_fixedstep.key_states[i];
            input_state_fixedstep.key_states[i] = input_state_varstep.key_states[i];
        }
    }

    user.fixed_step(shared_data, fixed_step_data).await;
}

pub(crate) async fn vary_tick<T: User>(shared_data: Arc<SharedAppData>, variable_step_data: Arc<VariableStepData>, user: Arc<T>) {
    {
        profiling::scope!("vary_tick before user");
    }

    {
        let input_state_backbuffer = &mut*variable_step_data.input_state_backbuffer.lock().await;
        let input_state_frontbuffer = &mut*variable_step_data.input_state_frontbuffer.lock().await;
        for i in 0..512 {
            input_state_frontbuffer.key_states_old[i] = input_state_backbuffer.key_states_old[i];
            input_state_frontbuffer.key_states[i] = input_state_backbuffer.key_states[i];
        }
    }

    user.varaible_step(shared_data, variable_step_data.clone()).await;
    variable_step_data.renderer.render().await.unwrap();
     
    {
        let input_state_backbuffer = &mut*variable_step_data.input_state_backbuffer.lock().await;
        for i in 0..512 {
            input_state_backbuffer.key_states_old[i] = input_state_backbuffer.key_states[i];
        }
    }
}