use std::{cell::RefCell, pin::Pin, sync::Arc, task::{Context, Poll}};

use futures::{future::*, task::{ArcWake, waker_ref}};

struct UnblockSignal{}

struct BlockedThreadWaker{
    snd: crossbeam_channel::Sender<UnblockSignal>,
}

impl ArcWake for BlockedThreadWaker {
    fn wake(self: Arc<Self>) {
        self.snd.send(UnblockSignal{}).unwrap();
    }

    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.snd.send(UnblockSignal{}).unwrap();
    }
}

struct LocalBlockData {
    send: crossbeam_channel::Sender<UnblockSignal>,
    recv: crossbeam_channel::Receiver<UnblockSignal>,
    waker: Arc<BlockedThreadWaker>,
}

impl LocalBlockData {
    fn new() -> Self {
        let (send, recv) = crossbeam_channel::bounded(1);
        Self{
            send: send.clone(),
            recv,
            waker: Arc::new(BlockedThreadWaker{snd:send})
        }
    }
}

thread_local! {
    static LOCAL_BLOCK_ON_DATA: RefCell<LocalBlockData>  = RefCell::new(LocalBlockData::new());
}

#[allow(unused)]
pub fn block_on<Out>(mut future: impl Future<Output = Out>) -> Out {

    LOCAL_BLOCK_ON_DATA.with(
        |ref_cell| {
            let local_block_data = ref_cell.borrow_mut();
            let waker = waker_ref(&local_block_data.waker);
            let context = &mut Context::from_waker(&*waker);

            local_block_data.send.try_send(UnblockSignal{});

            loop {
                let _ = local_block_data.recv.recv().unwrap();
                let future = unsafe{ Pin::new_unchecked(&mut future)};
                if let Poll::Ready(val) = future.poll(context) {
                    break val;
                }
            }
        }
    )
}