use crate::output::dumb::DumbLogger;
use crate::output::interface::Logger;
use std::time::Duration;

#[test]
fn test() {
    let logger = DumbLogger::new();

    let logger = logger.start("Ruby Buildpack");

    // Doesnt compile! yay!
    // logger.step("asd");

    let logger = logger.section("Ruby Version 1.2.3");
    logger.step("Downloading tar");
    logger.step("Determining install method");

    let logger = logger.step_timed("extracting tarball");

    // Simulate work...
    std::thread::sleep(Duration::from_secs(10));

    let logger = logger.finish_timed_step();
    let logger = logger.end_section();

    let logger = logger.section("Celebration");
    logger
        .step("Getting wine")
        .step("Grabbing glasses")
        .step("Drinking");

    logger.end_section().finish_logging();
}
