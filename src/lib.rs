pub use std::sync::mpsc::{SendError, Sender};
pub use std::sync::Arc;

use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::thread::{spawn, JoinHandle};

pub struct MPSCQueue<T: Sized> {
    thread_handle: JoinHandle<()>,
    queue_sender: Sender<Option<T>>,
    shutdown_flag: Arc<AtomicBool>,
    phantom: PhantomData<T>,
}

impl<T> MPSCQueue<T>
where
    T: Send + Sized,
{
    pub fn start(
        new_queue_handler: Arc<&'static (dyn Fn(T) + Send + Sync)>,
    ) -> (Sender<Option<T>>, Self) {
        let (queue_sender, queue_receiver) = channel();
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let shutdown_flag_clone = shutdown_flag.clone();
        let handler_clone = new_queue_handler.clone();
        let thread_handle = spawn(move || loop {
            if shutdown_flag_clone.load(Ordering::Relaxed) {
                break;
            }
            match queue_receiver.recv() {
                Ok(new_data) => match new_data {
                    Some(valid_data) => handler_clone(valid_data),
                    None => break,
                },
                Err(_) => break,
            }
        });
        (
            queue_sender.clone(),
            Self {
                thread_handle,
                shutdown_flag,
                queue_sender,
                phantom: PhantomData,
            },
        )
    }

    pub fn shutdown(self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
        let _ = self.queue_sender.send(None);
        let _ = self.thread_handle.join();
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn do_nothing_handler(_: u64) {
        ()
    }

    #[test]
    fn shutdown_test() {
        let (queue_tx, mpsc_instance) = MPSCQueue::start(Arc::new(&do_nothing_handler));
        for i in 0..100_000 {
            let _ = queue_tx.send(Some(i));
        }
        mpsc_instance.shutdown();
        assert!(true);
    }
}
