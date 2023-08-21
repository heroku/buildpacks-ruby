use std::io::Write;
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub fn start_timer<T>(
    arc_io: &Arc<Mutex<T>>,
    start: impl AsRef<str>,
    tick: impl AsRef<str>,
    end: impl AsRef<str>,
) -> StopJoinGuard<StopTimer>
where
    // The 'static lifetime means as long as something holds a reference to it, nothing it references
    // will go away.
    //
    // From https://users.rust-lang.org/t/why-does-thread-spawn-need-static-lifetime-for-generic-bounds/4541
    //
    //   [lifetimes] refer to the minimum possible lifetime of any borrowed references that the object contains.
    T: Write + Send + Sync + 'static,
{
    let instant = Instant::now();
    let (sender, receiver) = mpsc::channel::<()>();
    let start = start.as_ref().to_string();
    let tick = tick.as_ref().to_string();
    let end = end.as_ref().to_string();

    let arc_io = arc_io.clone();
    let handle = std::thread::spawn(move || {
        let mut io = arc_io.lock().unwrap();
        write!(&mut io, "{start}").expect("Internal error");
        io.flush().expect("Internal error");
        loop {
            write!(&mut io, "{tick}").expect("Internal error");
            io.flush().expect("Internal error");

            if receiver.recv_timeout(Duration::from_secs(1)).is_ok() {
                write!(&mut io, "{end}").expect("Internal error");
                io.flush().expect("Internal error");
                break;
            }
        }
    });

    StopJoinGuard {
        inner: Some(StopTimer {
            handle: Some(handle),
            sender: Some(sender),
            instant,
        }),
    }
}

#[derive(Debug)]
pub struct StopTimer {
    instant: Instant,
    handle: Option<JoinHandle<()>>,
    sender: Option<Sender<()>>,
}

impl StopTimer {
    pub fn elapsed(&self) -> Duration {
        self.instant.elapsed()
    }
}

pub trait StopJoin: std::fmt::Debug {
    fn stop_join(self) -> Self;
}

impl StopJoin for StopTimer {
    fn stop_join(mut self) -> Self {
        if let Some(inner) = self.sender.take() {
            inner.send(()).expect("Internal error");
        }

        if let Some(inner) = self.handle.take() {
            inner.join().expect("Internal error");
        }

        self
    }
}

// Guarantees that stop is called on the inner
//
// Expects and inner to return a Duration
#[derive(Debug)]
pub struct StopJoinGuard<T: StopJoin> {
    inner: Option<T>,
}

impl<T: StopJoin> StopJoinGuard<T> {
    pub fn stop(mut self) -> Option<T> {
        self.inner.take().map(StopJoin::stop_join)
    }
}

impl<T: StopJoin> Drop for StopJoinGuard<T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.stop_join();
        }
    }
}
