use std::{cell::RefCell, pin::Pin, sync::Arc, task::{Context, Poll}};

use futures::Future;

use super::task::Task;

pub(crate) struct YieldInfo {
    pub(crate) did_yield: bool,
    pub(crate) yield_task: Option<Arc<Task>>,
}

thread_local! {
    pub(crate) static YIELD_INFO: RefCell<YieldInfo> = RefCell::new(YieldInfo{did_yield:false, yield_task: None});
}

struct YieldFuture {
    yielded_once: bool,
}

impl Future for YieldFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded_once {
            return Poll::Ready(());
        } 
        self.yielded_once = true;

        YIELD_INFO.with(|d|{
            d.borrow_mut().did_yield = true;
        });

        Poll::Pending
    }
}

#[allow(unused)]
pub async fn yield_now() {
    YieldFuture{yielded_once:false}.await;
}