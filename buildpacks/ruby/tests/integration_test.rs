// Required due to: https://github.com/rust-lang/rust/issues/95513
#![allow(unused_crate_dependencies)]
// Required due to: https://github.com/rust-lang/rust-clippy/issues/11119
#![allow(clippy::unwrap_used)]

use indoc::{formatdoc, indoc};
use libcnb_test::{
    assert_contains, assert_contains_match, assert_empty, BuildConfig, BuildpackReference,
    ContainerConfig, ContainerContext, TestRunner,
};
use std::thread;
use std::time::{Duration, Instant};
use ureq::Response;

// Test that:
// - Cached data "stack" is preserved and will be successfully migrated to "targets"
#[test]
#[ignore = "integration test"]
fn test_migrating_metadata_or_layer_names() {
    // This test is a placeholder for when a change modifies metadata structures.
    // Remove the return and update the `buildpack-ruby` reference to the latest version.
    #![allow(unreachable_code)]
    // Test v5.0.1 compatible with v5.0.0

    let builder = "heroku/builder:24";
    let app_dir = "tests/fixtures/default_ruby";

    TestRunner::default().build(
        BuildConfig::new(builder, app_dir).buildpacks([BuildpackReference::Other(
            "docker://docker.io/heroku/buildpack-ruby:5.0.0".to_string(),
        )]),
        |context| {
            println!("{}", context.pack_stdout);
            context.rebuild(
                BuildConfig::new(builder, app_dir).buildpacks([BuildpackReference::CurrentCrate]),
                |rebuild_context| {
                    println!("{}", rebuild_context.pack_stdout);

                    assert_contains_match!(
                        rebuild_context.pack_stdout,
                        r"^- Ruby version[^\n]*\n  - Using cache"
                    );
                    assert_contains_match!(
                        rebuild_context.pack_stdout,
                        r"^- Bundler version[^\n]*\n  - Using cache"
                    );
                    assert_contains_match!(
                        rebuild_context.pack_stdout,
                        r"^- Bundle install gems[^\n]*\n  - Using cache"
                    );
                },
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn test_default_app_ubuntu20() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:20", "tests/fixtures/default_ruby"),
        |context| {
            println!("{}", context.pack_stdout);
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");
            assert_contains!(
                context.pack_stdout,
                r#"`BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install`"#);

            assert_contains!(context.pack_stdout, "Installing puma");

        // Check that at run-time:
        // - The correct env vars are set.
        let command_output = context.run_shell_command(
            indoc! {"
                set -euo pipefail
                printenv | sort | grep -vE '(_|HOME|HOSTNAME|OLDPWD|PWD|SHLVL|SECRET_KEY_BASE)='

                # Output command + output to stdout
                export BASH_XTRACEFD=1; set -o xtrace
                which -a rake
                which -a ruby
            "}
        );
        assert_empty!(command_output.stderr);
        assert_eq!(
            command_output.stdout,
            formatdoc! {"
                BUNDLE_BIN=/layers/heroku_ruby/gems/bin
                BUNDLE_CLEAN=1
                BUNDLE_DEPLOYMENT=1
                BUNDLE_GEMFILE=/workspace/Gemfile
                BUNDLE_PATH=/layers/heroku_ruby/gems
                BUNDLE_WITHOUT=development:test
                DISABLE_SPRING=1
                GEM_PATH=/layers/heroku_ruby/gems:/layers/heroku_ruby/bundler
                JRUBY_OPTS=-Xcompile.invokedynamic=false
                LD_LIBRARY_PATH=/layers/heroku_ruby/binruby/lib
                MALLOC_ARENA_MAX=2
                PATH=/layers/heroku_ruby/bundler/bin:/layers/heroku_ruby/gems/bin:/layers/heroku_ruby/bundler/bin:/layers/heroku_ruby/binruby/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
                RACK_ENV=production
                RAILS_ENV=production
                RAILS_LOG_TO_STDOUT=enabled
                RAILS_SERVE_STATIC_FILES=enabled
                + which -a rake
                /layers/heroku_ruby/gems/bin/rake
                /layers/heroku_ruby/binruby/bin/rake
                /usr/bin/rake
                /bin/rake
                + which -a ruby
                /layers/heroku_ruby/binruby/bin/ruby
                /usr/bin/ruby
                /bin/ruby
            "}
        );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn test_default_app_ubuntu22() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "tests/fixtures/default_ruby"),
        |context| {
            println!("{}", context.pack_stdout);
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");
            assert_contains!(
                context.pack_stdout,
                r#"`BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install`"#);

            assert_contains!(context.pack_stdout, "Installing puma");
        },
    );
}

#[test]
#[ignore = "integration test"]
fn test_default_app_latest_distro() {
    let config = amd_arm_builder_config("heroku/builder:24", "tests/fixtures/default_ruby");

    TestRunner::default().build(
        config,
        |context| {
            println!("{}", context.pack_stdout);
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");
            assert_contains!(
                context.pack_stdout,
                r#"`BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install`"#);

            assert_contains!(context.pack_stdout, "Installing puma");

            let secret_key_base = context.run_shell_command("echo \"${SECRET_KEY_BASE:?No SECRET_KEY_BASE set}\"").stdout;
            assert!(!secret_key_base.trim().is_empty(), "Expected {secret_key_base:?} to not be empty but it is");

            let config = context.config.clone();
            context.rebuild(config, |rebuild_context| {
                println!("{}", rebuild_context.pack_stdout);
                assert_contains!(rebuild_context.pack_stdout, "Skipping `bundle install` (no changes found in /workspace/Gemfile, /workspace/Gemfile.lock, or user configured environment variables)");

                rebuild_context.start_container(
                    ContainerConfig::new()
                        .env("PORT", TEST_PORT.to_string())
                        .expose_port(TEST_PORT),
                    |container| {
                        let response = call_root_until_boot(&container, TEST_PORT).unwrap();
                        let body = response.into_string().unwrap();

                        let server_logs = container.logs_now();

                        assert_contains!(server_logs.stdout, "Puma starting");
                        assert_empty!(server_logs.stderr);

                        assert_contains!(body, "ruby_version");
                    },
                );

                // Assert SECRET_KEY_BASE is preserved between invocations
                assert_eq!(
                    secret_key_base,
                    rebuild_context.run_shell_command("echo \"${SECRET_KEY_BASE:?No SECRET_KEY_BASE set}\"").stdout
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

        ruby '3.1.4', engine: 'jruby', engine_version: '9.4.8.0'
    "#,
    )
    .unwrap();

    fs_err::write(
        app_dir.path().join("Gemfile.lock"),
        r"
GEM
  remote: https://rubygems.org/
  specs:
PLATFORMS
  java
RUBY VERSION
   ruby 3.1.4p001 (jruby 9.4.8.0)
DEPENDENCIES
",
    )
    .unwrap();

    let mut config = amd_arm_builder_config("heroku/builder:24", &app_dir.path().to_string_lossy());

    TestRunner::default().build(
        config
        .buildpacks([
            BuildpackReference::Other(String::from("heroku/jvm")),
            BuildpackReference::CurrentCrate,
        ]),
        |context| {
            println!("{}", context.pack_stdout);
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");
            assert_contains!(
                context.pack_stdout,
                r#"`BUNDLE_BIN="/layers/heroku_ruby/gems/bin" BUNDLE_CLEAN="1" BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_PATH="/layers/heroku_ruby/gems" BUNDLE_WITHOUT="development:test" bundle install`"#
            );
            assert_contains!(context.pack_stdout, "Ruby version `3.1.4-jruby-9.4.8.0` from `Gemfile.lock`");
            });
}

#[test]
#[ignore = "integration test"]
fn test_ruby_app_with_yarn_app() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "tests/fixtures/yarn-ruby-app")
        .buildpacks([
            BuildpackReference::Other(String::from("heroku/nodejs-engine")),
            BuildpackReference::Other(String::from("heroku/nodejs-yarn")),
            BuildpackReference::CurrentCrate,
        ]),
        |context| {
            println!("{}", context.pack_stdout);
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
        BuildConfig::new("heroku/builder:22", "tests/fixtures/barnes_app"),
        |context| {
            println!("{}", context.pack_stdout);

            assert_contains!(context.pack_stdout, "Installing metrics agent from https://agentmon-releases.s3.us-east-1.amazonaws.com/agentmon");
            context.start_container(
                ContainerConfig::new()
                    .entrypoint("launcher")
                    .envs([
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

fn time_bounded_retry<T, E, F>(max_time: Duration, sleep_for: Duration, f: F) -> Result<T, E>
where
    F: Fn() -> Result<T, E>,
{
    let start = Instant::now();

    loop {
        let result = f();
        if result.is_ok() || max_time <= (start.elapsed() + sleep_for) {
            return result;
        }
        thread::sleep(sleep_for);
    }
}

fn call_root_until_boot(
    container: &ContainerContext,
    port: u16,
) -> Result<Response, Box<ureq::Error>> {
    let response = time_bounded_retry(Duration::from_secs(10), frac_seconds(0.1_f64), || {
        request_container(container, port, "")
    });

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

// TODO: Once Pack build supports `--platform` and libcnb-test adjusted accordingly, change this
// to allow configuring the target arch independently of the builder name (eg via env var).
fn amd_arm_builder_config(builder_name: &str, app_dir: &str) -> BuildConfig {
    let mut config = BuildConfig::new(builder_name, app_dir);

    match builder_name {
        "heroku/builder:24" if cfg!(target_arch = "aarch64") => {
            config.target_triple("aarch64-unknown-linux-musl")
        }
        _ => config.target_triple("x86_64-unknown-linux-musl"),
    };
    config
}
