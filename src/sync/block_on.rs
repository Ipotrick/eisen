use std::{pin::Pin, sync::Arc, task::{Context, Poll}};

use futures::{future::*, task::{ArcWake, waker_ref}};

struct BlockedThreadWaker{
    snd: crossbeam_channel::Sender<bool>,
}

impl ArcWake for BlockedThreadWaker {
    fn wake(self: Arc<Self>) {
        self.snd.send(true).unwrap();
    }

    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.snd.send(true).unwrap();
    }
}

#[allow(unused)]
pub fn block_on<Out>(mut future: impl Future<Output = Out>) -> Out {
    let (snd,rcv) = crossbeam_channel::unbounded::<bool>();
    snd.send( true ).unwrap();

    let waker = Arc::new(BlockedThreadWaker{snd:snd});
    let waker = waker_ref(&waker);
    let context = &mut Context::from_waker(&*waker);

    loop {
        let _ = rcv.recv().unwrap();
        let future = unsafe{ Pin::new_unchecked(&mut future)};
        if let Poll::Ready(val) = future.poll(context) {
            break val;
        }
    }
}