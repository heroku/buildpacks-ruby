use std::io::Write;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

fn start_timer<D: Write + Send>(
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
        let dest = start_timer(Vec::<u8>::new(), "Installing", || {
            sleep(Duration::from_secs(2));
        });
        assert_eq!("Installing .... (2s)", String::from_utf8_lossy(&dest));
    }
}
