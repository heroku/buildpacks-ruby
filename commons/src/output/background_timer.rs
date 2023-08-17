use std::fmt::Display;
use std::io::{Stdout, Write};
use std::ops::DerefMut;
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, Instant};

struct StopTimer {
    instant: Instant,
    handle: JoinHandle<()>,
    sender: Sender<()>,
    destination: Arc<Mutex<LogBackend>>,
}

impl StopTimer {
    fn stop(mut self) -> LogBackend {
        self.sender.send(()).expect("Internal error");
        self.handle.join().expect("Internal error");

        let time = self.instant.elapsed().as_secs();
        let mut destination = Arc::try_unwrap(self.destination)
            .expect("Internal error")
            .into_inner()
            .expect("Internal error");

        destination.write_now(format!("({time}s)\n"));
        destination
    }
}

struct StopOrDrop {
    inner: Option<StopTimer>,
}

impl StopOrDrop {
    fn stop(&mut self) -> Option<LogBackend> {
        self.inner.take().map(StopTimer::stop)
    }
}

impl Drop for StopOrDrop {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug)]
enum LogBackend {
    Stdout(Stdout),
    Memory(Vec<u8>),
}

impl LogBackend {
    fn write_now(&mut self, s: impl AsRef<str>) {
        match self {
            LogBackend::Stdout(out) => write_now(out, s.as_ref()),
            LogBackend::Memory(out) => write_now(out, s.as_ref()),
        }
    }
}

impl Display for LogBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogBackend::Stdout(_) => Ok(()),
            LogBackend::Memory(m) => f.write_str(&String::from_utf8_lossy(m)),
        }
    }
}

fn start_timer(mut destination: LogBackend, start_message: impl AsRef<str>) -> StopTimer {
    destination.write_now(start_message);
    let arc_destination = Arc::new(Mutex::new(destination));
    let instant = Instant::now();
    let (sender, receiver) = mpsc::channel::<()>();

    let thread_destination = arc_destination.clone();
    let handle = std::thread::spawn(move || {
        let mut destination = thread_destination.lock().unwrap();
        destination.write_now(" .");
        loop {
            match receiver.recv_timeout(Duration::from_secs(1)) {
                Ok(_) => {
                    destination.write_now(". ");
                    break;
                }
                Err(_) => destination.write_now("."),
            }
        }
    });

    StopTimer {
        handle,
        sender,
        instant,
        destination: arc_destination.clone(),
    }
}

fn start_timer_scoped<D: Write + Send>(
    mut destination: D,
    start_message: impl AsRef<str>,
    f: impl FnOnce(),
) -> D {
    let (sender, receiver) = mpsc::channel::<()>();
    write_now(&mut destination, start_message);
    let timer = Instant::now();

    std::thread::scope(|s| {
        let handle = s.spawn(move || {
            write_now(&mut destination, " .");
            loop {
                match receiver.recv_timeout(Duration::from_secs(1)) {
                    Ok(_) => {
                        write_now(&mut destination, ". ");
                        break;
                    }
                    Err(_) => write_now(&mut destination, "."),
                }
            }
            destination
        });

        f();
        sender.send(()).expect("Internal error: channel is closed");

        let mut destination = handle
            .join()
            .expect("Internal error: UI thread unexpectedly errored");

        write_now(
            &mut destination,
            format!("({:?}s)", timer.elapsed().as_secs()),
        );
        destination
    })
}

fn write_now<D: Write>(destination: &mut D, msg: impl AsRef<str>) {
    write!(destination, "{}", msg.as_ref()).expect("Internal error: UI writer closed");

    destination
        .flush()
        .expect("Internal error: UI writer closed");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lol() {
        let timer = start_timer(LogBackend::Memory(Vec::new()), "Installing");
        sleep(Duration::from_secs(2));
        let dest = timer.stop();

        assert_eq!("Installing ... (2s)\n", &dest.to_string());
    }
}
