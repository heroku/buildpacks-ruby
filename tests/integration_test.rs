#![warn(clippy::pedantic)]

use libcnb_test::{assert_contains, IntegrationTest};
use std::io;

#[test]
#[ignore]
fn test() {
    IntegrationTest::new("heroku/buildpacks:20", "tests/fixtures/default_ruby")
        .run_test(|context| {
            // On failure, print the stdout
            println!("{}", context.pack_stdout);

            assert!(context
                .pack_stdout
                .contains("---> Download and extracting Ruby"));
            assert!(context.pack_stdout.contains("Running: bundle install"));

            context
                .prepare_container()
                .env("PORT", TEST_PORT.to_string())
                .expose_port(TEST_PORT)
                .start_with_default_process(|container| {
                    std::thread::sleep(std::time::Duration::from_secs(1));

                    let result =
                        call_test_fixture_service(container.address_for_port(TEST_PORT).unwrap())
                            .unwrap();

                    println!("{}", result);
                    assert_contains!(result, "ruby_version");
                });
        });
}

fn call_test_fixture_service(addr: std::net::SocketAddr) -> io::Result<String> {
    let req = ureq::get(&format!("http://{}:{}/", addr.ip(), addr.port(),));
    req.call().unwrap().into_string()
}

const TEST_PORT: u16 = 12346;
