use std::{pin::Pin, task::{Context, Poll}};

use futures::Future;

struct YieldFuture {
    yielded_once: bool,
}

impl Future for YieldFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded_once {
            return Poll::Ready(());
        } 
        self.yielded_once = true;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

#[allow(unused)]
pub async fn yield_now() {
    YieldFuture{yielded_once:false}.await;
}