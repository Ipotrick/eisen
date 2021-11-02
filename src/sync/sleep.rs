use std::{pin::Pin, task::{Context, Poll, Waker}};

use super::yielding::*;
use futures::Future;

use super::Runtime;


const SLEEP_SPIN_TIME: std::time::Duration = std::time::Duration::from_micros(100);

pub(crate) struct SleepFuture {
    called_once: bool,
    wake_up_time: std::time::Instant,
    sender: async_std::channel::Sender<SleepingTask>,
}

pub(crate) struct SleepingTask {
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

pub(crate) async fn sleep_sheduler(sleepers_rcv: async_std::channel::Receiver<SleepingTask>) {
    let mut sleepers = Vec::new();
    loop {
        if sleepers.is_empty() {
            if let Ok(sleeper) = sleepers_rcv.recv().await {
                sleepers.push(sleeper);
            } else {
                break;
            }
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

#[allow(unused)]
pub async fn sleep_for(runtime: &Runtime, min_dura: std::time::Duration) {
    let wake_up_time = std::time::Instant::now() + min_dura;
    sleep_until(runtime, wake_up_time).await
}

#[allow(unused)]
pub async fn sleep_until(runtime: &Runtime, wake_up_time: std::time::Instant) {
    let sleeper = SleepFuture{
        called_once: false,
        wake_up_time: wake_up_time,
        sender: runtime.meta.new_sleepers_snd.clone(),
    };

    sleeper.await;
}