pub use std::sync::mpsc::{SendError, Sender};
pub use std::sync::Arc;

use std::marker::PhantomData;
use std::sync::mpsc::channel;
use std::thread::{spawn, JoinHandle};

pub struct MPSCQueue<T: Sized> {
    thread_handle: JoinHandle<()>,
    phantom: PhantomData<T>,
}

impl<T> MPSCQueue<T>
where
    T: Send + Sized,
{
    pub fn start(new_queue_handler: Arc<&'static (dyn Fn(T) + Send + Sync)>) -> (Sender<T>, Self) {
        let (queue_sender, queue_receiver) = channel();
        let handler_clone = new_queue_handler.clone();
        let thread_handle = spawn(move || loop {
            match queue_receiver.recv() {
                Ok(new_data) => handler_clone(new_data),
                Err(_) => break,
            }
        });
        (
            queue_sender,
            Self {
                thread_handle,
                phantom: PhantomData,
            },
        )
    }

    pub fn shutdown(self, returned_senders: Vec<Sender<T>>) {
        let mut senders = returned_senders;
        while senders.pop().is_some() {}
        let _ = self.thread_handle.join();
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const LOOP_COUNT: u64 = 10_000;
    static RESULT: AtomicU64 = AtomicU64::new(0);

    fn do_nothing_handler(value: u64) {
        RESULT.store(value, Ordering::Relaxed);
    }

    #[test]
    fn shutdown_test() {
        let (queue_tx, mpsc_instance) = MPSCQueue::start(Arc::new(&do_nothing_handler));
        for i in 0..LOOP_COUNT {
            let _ = queue_tx.send(i);
        }
        mpsc_instance.shutdown(vec![queue_tx]);
        assert_eq!(RESULT.load(Ordering::Relaxed), LOOP_COUNT - 1);
    }
}
