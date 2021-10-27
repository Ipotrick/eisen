#![macro_use]

use std::{pin::Pin, sync::{Arc, Mutex, atomic::AtomicUsize}, task::{Context, Poll, Waker}};

use futures::{Future};

struct SharedData {
    count: AtomicUsize,
    waker: Mutex<Option<Waker>>,
}

pub struct AtomicDependency{
    data: Arc<SharedData>,
}

impl Clone for AtomicDependency{
    fn clone(&self) -> Self {
        self.data.count.fetch_add(1, std::sync::atomic::Ordering::Acquire);
        Self{
            data: self.data.clone(),
        }
    }
}

impl Drop for AtomicDependency {
    fn drop(&mut self) {
        let count = self.data.count.fetch_sub(1, std::sync::atomic::Ordering::Acquire);
        if count == 1 {
            if let Some(waker) = &*self.data.waker.lock().unwrap() {
                waker.wake_by_ref();
            }
        }
    }
}

pub struct AtomicWaiter {
    data: Arc<SharedData>,
}

impl AtomicWaiter {
    #[allow(unused)]
    pub fn new() -> Self {
        Self {
            data: Arc::new(SharedData{count: AtomicUsize::new(0), waker: Mutex::new(None)}),
        }
    }

    #[allow(unused)]
    pub fn make_dependency(&self) -> AtomicDependency {
        self.data.count.fetch_add(1, std::sync::atomic::Ordering::Acquire);
        AtomicDependency{
            data: self.data.clone(),
        }
    }
}

impl Future for AtomicWaiter {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.data.count.fetch_add(0, std::sync::atomic::Ordering::Acquire) == 0 {
            return Poll::Ready(());
        }
        let mut waker = self.data.waker.lock().unwrap();
        if waker.is_none() {
            *waker = Some(cx.waker().clone());
        }
        Poll::Pending
    }
}

impl Drop for AtomicWaiter {
    fn drop(&mut self) {
        if self.data.count.fetch_add(0, std::sync::atomic::Ordering::Acquire) != 0 {
        }
    }
}

#[allow(unused)]
macro_rules! spin_on {
    ($expression:expr) => {
        loop {
            if let Some(guard) = $expression {
                break guard
            }
        }
    };
}
#[allow(unused)]
pub(crate) use spin_on;