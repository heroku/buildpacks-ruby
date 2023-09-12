#![warn(clippy::pedantic)]

use libcnb_test::{
    assert_contains, assert_empty, BuildConfig, BuildpackReference, ContainerConfig,
    ContainerContext, TestRunner,
};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::__private::DisplayAsDisplay;
use ureq::Response;

#[test]
#[ignore = "integration test"]
fn test_default_app() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "tests/fixtures/default_ruby")
        .buildpacks(vec![BuildpackReference::Crate]),
        |context| {
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");
            assert_contains!(
                context.pack_stdout,
                r#"`BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install`"#);

            println!("{}", context.pack_stdout); // Needed to get full failure as `rebuild` truncates stdout
            assert_contains!(context.pack_stdout, "Installing webrick");

            let config = context.config.clone();
            context.rebuild(config, |rebuild_context| {
                assert_contains!(rebuild_context.pack_stdout, "Skipping `bundle install` (no changes found in /workspace/Gemfile, /workspace/Gemfile.lock, or user configured environment variables)");

                rebuild_context.start_container(
                    ContainerConfig::new()
                        .env("PORT", TEST_PORT.to_string())
                        .expose_port(TEST_PORT),
                    |container| {
                        let response = call_root_until_boot(&container, TEST_PORT).unwrap();
                        let body = response.into_string().unwrap();

                        let server_logs = container.logs_now();
                        assert_contains!(server_logs.stderr, "WEBrick::HTTPServer#start");
                        assert_empty!(server_logs.stdout);

                        assert_contains!(body, "ruby_version");
                    },
                );
            });
        },
    );
}

#[test]
#[ignore = "integration test"]
fn test_jruby_app() {
    let app_dir = tempfile::tempdir().unwrap();
    fs_err::write(
        app_dir.path().join("Gemfile"),
        r#"
        source "https://rubygems.org"

        ruby '2.6.8', engine: 'jruby', engine_version: '9.3.6.0'
    "#,
    )
    .unwrap();

    fs_err::write(
        app_dir.path().join("Gemfile.lock"),
        r#"
GEM
  remote: https://rubygems.org/
  specs:
PLATFORMS
  java
RUBY VERSION
   ruby 2.6.8p001 (jruby 9.3.6.0)
DEPENDENCIES
"#,
    )
    .unwrap();

    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", app_dir.path())
        .buildpacks(vec![
            BuildpackReference::Other(String::from("heroku/jvm")),
            BuildpackReference::Crate,
        ]),
        |context| {
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");
            assert_contains!(
                context.pack_stdout,
                r#"`BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install`"#
            );
            assert_contains!(context.pack_stdout, "Ruby version `2.6.8-jruby-9.3.6.0` from `Gemfile.lock`");
            });
}

#[test]
#[ignore = "integration test"]
fn test_ruby_app_with_yarn_app() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "tests/fixtures/yarn-ruby-app")
        .buildpacks(vec![
            BuildpackReference::Other(String::from("heroku/nodejs-engine")),
            BuildpackReference::Other(String::from("heroku/nodejs-yarn")),
            BuildpackReference::Crate,
        ]),
        |context| {
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");
            assert_contains!(
                context.pack_stdout,
                r#"`BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install`"#);
            }
        );
}

#[test]
#[ignore = "integration test"]
fn test_barnes_app() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "tests/fixtures/barnes_app")
            .buildpacks(vec![BuildpackReference::Crate]),
        |context| {
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");

            context.start_container(
                ContainerConfig::new()
                    .entrypoint("launcher")
                    .envs(vec![
                        ("DYNO", "web.1"),
                        ("PORT", "1234"),
                        ("AGENTMON_DEBUG", "1"),
                        ("HEROKU_METRICS_URL", "example.com"),
                    ])
                    .command(["while true; do sleep 1; done"]),
                |container| {
                    let boot_message = "Booting agentmon_loop";
                    let mut agentmon_log = String::new();

                    let started = Instant::now();
                    while started.elapsed() < Duration::from_secs(20) {
                        if agentmon_log.contains(boot_message) {
                            break;
                        }

                        std::thread::sleep(frac_seconds(0.1));
                        agentmon_log = container
                            .shell_exec("cat /layers/heroku_ruby/metrics_agent/output.log")
                            .stdout;
                    }

                    let log_output = container.logs_now();
                    println!("{}", log_output.stdout);
                    println!("{}", log_output.stderr);

                    assert_contains!(agentmon_log, boot_message);
                },
            );
        },
    );
}

fn request_container(
    container: &ContainerContext,
    port: u16,
    path: &str,
) -> Result<Response, Box<ureq::Error>> {
    let addr = container.address_for_port(port);
    let ip = addr.ip();
    let port = addr.port();
    let req = ureq::get(&format!("http://{ip}:{port}/{path}"));
    req.call().map_err(Box::new)
}

fn call_root_until_boot(
    container: &ContainerContext,
    port: u16,
) -> Result<Response, Box<ureq::Error>> {
    let mut count = 0;
    let max_time = 10.0_f64; // Seconds
    let sleep = 0.1_f64;

    #[allow(clippy::cast_possible_truncation)]
    let max_count = (max_time / sleep).floor() as i64;
    let mut response = request_container(container, port, "");
    while count < max_count {
        count += 1;
        match response {
            Err(ref box_e) => match box_e.as_ref() {
                ureq::Error::Transport(e) => {
                    println!(
                        "Waiting for connection {}, retrying in {}",
                        e.as_display(),
                        sleep
                    );
                    response = request_container(container, port, "");
                }
                ureq::Error::Status(..) => break,
            },
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
