pub use std::sync::mpsc::{SendError, Sender};
pub use std::sync::Arc;

use std::marker::PhantomData;
use std::sync::mpsc::channel;
use std::thread::{spawn, JoinHandle};

pub struct MPSCQConsumerWorker<T: Sized> {
    thread_handle: JoinHandle<()>,
    phantom: PhantomData<T>,
}

impl<T> MPSCQConsumerWorker<T>
where
    T: Send + Sized,
{
    pub fn start(new_queue_handler: Arc<&'static (dyn Fn(T) + Send + Sync)>) -> (Sender<T>, Self) {
        let (queue_sender, queue_receiver) = channel();
        let handler_clone = new_queue_handler.clone();
        let thread_handle = spawn(move || {
            while let Ok(new_data) = queue_receiver.recv() {
                handler_clone(new_data)
            }
        });
        (
            queue_sender,
            MPSCQConsumerWorker {
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
    const PRODUCER_COUNT: u8 = 4;

    #[test]
    fn single_producer_shutdown_test() {
        // Prepare test facilities
        static RESULT0: AtomicU64 = AtomicU64::new(0);
        fn global_store_handler(value: u64) {
            RESULT0.store(value, Ordering::Relaxed);
        }
        // Instantiate and start MPSCQueue consumer thread
        let (queue_tx, mpsc_instance) = MPSCQConsumerWorker::start(Arc::new(&global_store_handler));
        // Send data to MPSCQueue
        for i in 0..LOOP_COUNT {
            let _ = queue_tx.send(i);
        }
        // Shutdown MPSCQueue consumer by giving back the queue sender
        mpsc_instance.shutdown(vec![queue_tx]);
        // Consumer work result assertions
        assert_eq!(RESULT0.load(Ordering::Relaxed), LOOP_COUNT - 1);
    }

    #[test]
    fn multi_producers_shutdown_test() {
        // Prepare test facilities
        static RESULT1: AtomicU64 = AtomicU64::new(0);
        fn global_add_handler(value: u64) {
            RESULT1.fetch_add(value, Ordering::Relaxed);
        }
        // Instantiate and start MPSCQueue consumer thread
        let (queue_tx, mpsc_instance) = MPSCQConsumerWorker::start(Arc::new(&global_add_handler));
        // Declare common closure for sender/producer threads
        let loop_lambda = move |queue_tx: Sender<u64>| {
            for i in 0..LOOP_COUNT {
                let _ = queue_tx.send(i);
            }
        };
        // Spawn producer threads and collect its handles for joining
        let mut thread_handles: Vec<JoinHandle<()>> = Vec::new();
        for _ in 1..PRODUCER_COUNT {
            let tx_clone = queue_tx.clone();
            thread_handles.push(spawn(move || loop_lambda(tx_clone)));
        }
        thread_handles.push(spawn(move || loop_lambda(queue_tx)));
        // Shutdown MPSCQueue consumer thread, but gives no queue sender as its moved to
        // and automatically dropped by producer threads
        mpsc_instance.shutdown(Vec::new());
        // Gracefully waits for producer threads to finish their works
        while let Some(join_handle) = thread_handles.pop() {
            let _ = join_handle.join();
        }
        // Consumer work result assertions
        let expected_result: u64 = (LOOP_COUNT - 1) * LOOP_COUNT * (PRODUCER_COUNT as u64) / 2;
        assert_eq!(RESULT1.load(Ordering::Relaxed), expected_result);
    }
}
