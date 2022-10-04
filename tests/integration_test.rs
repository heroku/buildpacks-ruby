#![warn(clippy::pedantic)]

use libcnb_test::{
    assert_contains, assert_empty, BuildConfig, BuildpackReference, ContainerConfig, TestRunner,
};
use std::net::SocketAddr;
use std::time::Duration;
use std::{io, thread};

#[test]
#[ignore = "integration test"]
fn test_getting_started_rails_app() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "tests/fixtures/ruby-getting-started")
        .buildpacks(vec![BuildpackReference::Crate]),
        |context| {
            assert_contains!(context.pack_stdout, "---> Download and extracting Ruby");
            assert_contains!(
                context.pack_stdout,
                r#"Running: $ BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install"#
            );

            context.start_container(
                ContainerConfig::new()
                    .env("PORT", TEST_PORT.to_string())
                    .expose_port(TEST_PORT),
                |container| {
                    thread::sleep(Duration::from_secs(2));

                    let server_logs = container.logs_now();
                    assert_contains!(server_logs.stderr, "WEBrick::HTTPServer#start");
                    assert_empty!(server_logs.stdout);

                    let address_on_host = container.address_for_port(TEST_PORT).unwrap();
                    let response = call_test_fixture_service(address_on_host).unwrap();
                    assert_contains!(response, "ruby_version");
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
                r#"Running: $ BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install"#
            );

            context.start_container(
                ContainerConfig::new()
                    .env("PORT", TEST_PORT.to_string())
                    .expose_port(TEST_PORT),
                |container| {
                    thread::sleep(Duration::from_secs(2));

                    let server_logs = container.logs_now();
                    assert_contains!(server_logs.stderr, "WEBrick::HTTPServer#start");
                    assert_empty!(server_logs.stdout);

                    let address_on_host = container.address_for_port(TEST_PORT).unwrap();
                    let response = call_test_fixture_service(address_on_host).unwrap();
                    assert_contains!(response, "ruby_version");
                },
            );
        },
    );
}

fn call_test_fixture_service(addr: SocketAddr) -> io::Result<String> {
    let req = ureq::get(&format!("http://{}:{}/", addr.ip(), addr.port()));
    req.call().unwrap().into_string()
}

const TEST_PORT: u16 = 1234;
