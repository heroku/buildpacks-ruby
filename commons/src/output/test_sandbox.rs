// use crate::output::dumb::DumbLogger;
// use crate::output::interface::*;
// use std::time::Duration;

// #[test]
// fn test() {
//     let logger = DumbLogger::new();

//     let logger = logger.start("Ruby Buildpack");

//     // Doesnt compile! yay!
//     // logger.step("asd");

//     let logger = logger.section("Ruby Version 1.2.3");
//     logger.step("Downloading tar");
//     logger.step("Determining install method");

//     let logger = logger.step_timed("extracting tarball");

//     // Simulate work...
//     std::thread::sleep(Duration::from_secs(10));

//     let logger = logger.finish_timed_step();
//     let logger = logger.end_section();

//     let logger = logger.section("Celebration");
//     logger
//         .step("Getting wine")
//         .step("Grabbing glasses")
//         .step("Drinking");

//     logger.end_section().finish_logging();
// }

use std::io::Write;
use std::sync::{Arc, Mutex};

#[test]
fn lol() {
    let writer: Box<dyn Write + Send> = Box::new(std::io::stdout());

    let rocketship = Arc::new(Mutex::new(writer));
    let (sender, receiver) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let mut foo = rocketship.lock().unwrap();

        write!(foo, " .");
        loop {
            write!(foo, ".");

            if matches!(
                receiver.try_recv(),
                Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected)
            ) {
                write!(foo, " .");
                break;
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
    sender.send("done").unwrap();
}
