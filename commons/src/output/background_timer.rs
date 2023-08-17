use std::io::Write;
use std::mem::replace;
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub fn start_timer<T>(
    arc_io: &Arc<Mutex<T>>,
    start: impl AsRef<str>,
    tick: impl AsRef<str>,
    end: impl AsRef<str>,
) -> StopDrop<StopTimer>
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

    StopDrop {
        inner: RunState::Running(StopTimer {
            handle,
            sender,
            instant,
        }),
    }
}

#[derive(Debug)]
pub struct StopTimer {
    instant: Instant,
    handle: JoinHandle<()>,
    sender: Sender<()>,
}

pub trait StopIt: std::fmt::Debug {
    fn stop(self) -> Duration;
}

impl StopIt for StopTimer {
    fn stop(self) -> Duration {
        self.sender.send(()).expect("Internal error");
        self.handle.join().expect("Internal error");

        self.instant.elapsed()
    }
}

#[derive(Debug)]
enum RunState<T> {
    Running(T),
    Stopped(Duration),
}

// Guarantees that stop is called on the inner
//
// Expects and inner to return a Duration
#[derive(Debug)]
pub struct StopDrop<T: StopIt> {
    inner: RunState<T>,
}

impl<T: StopIt> StopDrop<T> {
    pub fn stop(&mut self) -> Duration {
        let inner = replace(&mut self.inner, RunState::Stopped(Default::default()));
        match inner {
            RunState::Running(obj) => {
                let duration = obj.stop();
                self.inner = RunState::Stopped(duration);
                duration
            }
            RunState::Stopped(duration) => duration,
        }
    }
}

impl<T: StopIt> Drop for StopDrop<T> {
    fn drop(&mut self) {
        self.stop();
    }
}
