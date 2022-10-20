#![warn(clippy::pedantic)]

use libcnb_test::{
    assert_contains, assert_empty, BuildConfig, BuildpackReference, ContainerConfig,
    ContainerContext, TestRunner,
};
use std::net::SocketAddr;
use std::time::Duration;
use std::{io, thread};
use ureq::Response;

#[test]
#[ignore = "integration test"]
fn test_getting_started_rails_app() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "tests/fixtures/ruby-getting-started")
        .buildpacks(vec![
            BuildpackReference::Other(String::from("heroku/nodejs-engine")),
            BuildpackReference::Crate,
            BuildpackReference::Other(String::from("heroku/procfile")),
        ]),
        |context| {
            assert_contains!(context.pack_stdout, "---> Download and extracting Ruby");
            assert_contains!(
                context.pack_stdout,
                r#"Running: $ BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install"#
            );
            println!("{}", context.pack_stdout);

            context.start_container(
                ContainerConfig::new()
                    .env("PORT", TEST_PORT.to_string())
                    .expose_port(TEST_PORT),
                |container| {
                    let boot = call_root_until_boot(&container, TEST_PORT);

                    let server_logs = container.logs_now();
                    assert_contains!(
                        server_logs.stdout.clone() + &server_logs.stderr,
                        "Puma starting"
                    );

                    boot.unwrap();

                    let address_on_host = container.address_for_port(TEST_PORT).unwrap();

                    let response = call_test_fixture_service(address_on_host).unwrap();
                    assert_contains!(response, "Getting Started with Ruby on Heroku");
                },
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn test_default_app() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "tests/fixtures/default_ruby")
        .buildpacks(vec![BuildpackReference::Crate]),
        |context| {
            assert_contains!(context.pack_stdout, "---> Download and extracting Ruby");
            assert_contains!(
                context.pack_stdout,
                r#"Running: $ BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install"#);

            assert_contains!(context.pack_stdout, "Installing webrick");

            let config = context.config.clone();
            context.rebuild(config, |rebuild_context| {
                assert_contains!(rebuild_context.pack_stdout, "Using webrick");

                rebuild_context.start_container(
                    ContainerConfig::new()
                        .env("PORT", TEST_PORT.to_string())
                        .expose_port(TEST_PORT),
                    |container| {
                        call_root_until_boot(&container, TEST_PORT).unwrap();

                        let server_logs = container.logs_now();
                        assert_contains!(server_logs.stderr, "WEBrick::HTTPServer#start");
                        assert_empty!(server_logs.stdout);

                        let address_on_host = container.address_for_port(TEST_PORT).unwrap();
                        let response = call_test_fixture_service(address_on_host).unwrap();
                        assert_contains!(response, "ruby_version");
                    },
                );
            });
        },
    );
}

fn call_test_fixture_service(addr: SocketAddr) -> io::Result<String> {
    let req = ureq::get(&format!("http://{}:{}/", addr.ip(), addr.port()));
    req.call().unwrap().into_string()
}

fn call_root_until_boot(container: &ContainerContext, port: u16) -> Result<Response, ureq::Error> {
    let mut count = 0;
    let max_time = 10.0; //Seconds
    let sleep = 0.1;

    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    let max_count = (max_time / sleep as f64).floor() as u64;
    let mut addr = container.address_for_port(port).unwrap();
    let mut req = ureq::get(&format!("http://{}:{}/", addr.ip(), addr.port()));
    let mut response = req.call();
    while count < max_count {
        count += 1;
        match response {
            Err(ureq::Error::Transport(e)) => {
                addr = container.address_for_port(port).unwrap();
                req = ureq::get(&format!("http://{}:{}/", addr.ip(), addr.port()));
                response = req.call();
                println!("Waiting for connection {}, retrying in {}", e, sleep);
            }
            _ => break,
        }

        thread::sleep(frac_seconds(sleep));
    }

    println!(
        "{}\n{}",
        container.logs_now().stdout,
        container.logs_now().stderr
    );
    response
}

fn frac_seconds(seconds: f64) -> Duration {
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    let value = (seconds * 1000.0).floor() as u64;
    Duration::from_millis(value)
}

const TEST_PORT: u16 = 1234;
