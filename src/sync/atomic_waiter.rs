
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
        {
            let mut waker = self.data.waker.lock().unwrap();
            *waker = Some(cx.waker().clone());
        }
        if self.data.count.load(std::sync::atomic::Ordering::Relaxed) == 0 {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Drop for AtomicWaiter {
    fn drop(&mut self) {
        if self.data.count.fetch_add(0, std::sync::atomic::Ordering::Acquire) != 0 {
        }
    }
}