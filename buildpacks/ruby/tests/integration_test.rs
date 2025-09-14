// Required due to: https://github.com/rust-lang/rust/issues/95513
#![allow(unused_crate_dependencies)]
// Required due to: https://github.com/rust-lang/rust-clippy/issues/11119
#![allow(clippy::unwrap_used)]

use indoc::{formatdoc, indoc};
use libcnb_test::{
    BuildConfig, BuildpackReference, ContainerConfig, ContainerContext, TestRunner,
    assert_contains, assert_contains_match, assert_empty,
};
use pretty_assertions::assert_eq;
use regex::Regex;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
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
    // Test v7.0.0 compatible with v6.0.0

    let builder = "heroku/builder:24";
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path();

    copy_dir_all(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("default_ruby"),
        app_dir,
    )
    .unwrap();

    // Specify explicit versions so changes in default values don't cause this test to fail
    writeln!(
        fs_err::OpenOptions::new()
            .write(true)
            .append(true)
            .open(app_dir.join("Gemfile.lock"))
            .unwrap(),
        indoc! {"
                RUBY VERSION
                   ruby 3.4.2
            "}
    )
    .unwrap();

    TestRunner::default().build(
        BuildConfig::new(builder, app_dir).buildpacks([BuildpackReference::Other(
            "docker://docker.io/heroku/buildpack-ruby:6.0.0".to_string(),
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
fn test_default_app_ubuntu22() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "tests/fixtures/default_ruby"),
        |context| {
            println!("{}", context.pack_stdout);
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");
            assert_contains!(
                context.pack_stdout,
                r#"`BUNDLE_FROZEN="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_WITHOUT="development:test" bundle install`"#
            );

            assert_contains!(context.pack_stdout, "Installing puma");
        },
    );
}

#[test]
#[ignore = "integration test"]
#[allow(clippy::too_many_lines)]
fn test_default_app_ubuntu24() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path();

    copy_dir_all(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("default_ruby"),
        app_dir,
    )
    .unwrap();
    let config = BuildConfig::new("heroku/builder:24", app_dir);
    TestRunner::default().build(
        config.clone(),
        |context| {
            println!("{}", context.pack_stdout);
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");
            assert_contains!(
                context.pack_stdout,
                r#"`BUNDLE_FROZEN="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_WITHOUT="development:test" bundle install`"#
            );

            assert_contains!(context.pack_stdout, "Installing puma");

        // Check that at run-time:
        // - The correct env vars are set.
        let command_output = context.run_shell_command(
            indoc! {"
                set -euo pipefail
                printenv | sort | grep -vE '(_|^HOME|HOSTNAME|OLDPWD|PWD|SHLVL|SECRET_KEY_BASE)='

                # Output command + output to stdout
                export BASH_XTRACEFD=1; set -o xtrace
                which -a rake
                which -a ruby
            "}
        );
        assert_empty!(command_output.stderr);
        assert_eq!(
            formatdoc! {"
                BUNDLE_FROZEN=1
                BUNDLE_GEMFILE=/workspace/Gemfile
                BUNDLE_WITHOUT=development:test
                DISABLE_SPRING=1
                GEM_HOME=/layers/heroku_ruby/gems
                GEM_PATH=/layers/heroku_ruby/gems:/layers/heroku_ruby/bundler
                JRUBY_OPTS=-Xcompile.invokedynamic=false
                LD_LIBRARY_PATH=/layers/heroku_ruby/binruby/lib
                MALLOC_ARENA_MAX=2
                PATH=/workspace/bin:/layers/heroku_ruby/bundler/bin:/layers/heroku_ruby/gems/bin:/layers/heroku_ruby/bundler/bin:/layers/heroku_ruby/binruby/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
                PUMA_PERSISTENT_TIMEOUT=95
                RACK_ENV=production
                RAILS_ENV=production
                RAILS_LOG_TO_STDOUT=enabled
                RAILS_SERVE_STATIC_FILES=enabled
                + which -a rake
                /layers/heroku_ruby/gems/bin/rake
                /layers/heroku_ruby/binruby/bin/rake
                + which -a ruby
                /layers/heroku_ruby/binruby/bin/ruby
            "},
            command_output.stdout,
        );

        fs_err::create_dir_all(app_dir.join("bin")).unwrap();
        fs_err::write(app_dir.join("bin").join("rake"), formatdoc!{r#"
            #!/usr/bin/env ruby
            # frozen_string_literal: true

            #
            # This file was generated by Bundler.
            #
            # The application 'rake' is installed as part of a gem, and
            # this file is here to facilitate running it.
            #

            ENV["BUNDLE_GEMFILE"] ||= File.expand_path("../Gemfile", __dir__)

            bundle_binstub = File.expand_path("bundle", __dir__)

            if File.file?(bundle_binstub)
                if File.read(bundle_binstub, 300).include?("This file was generated by Bundler")
                    load(bundle_binstub)
                else
                    abort("Your `bin/bundle` was not generated by Bundler, so this binstub cannot run.
                           Replace `bin/bundle` by running `bundle binstubs bundler --force`, then run this command again.")
                end
            end

            require "rubygems"
            require "bundler/setup"

            load Gem.bin_path("rake", "rake")
        "#}).unwrap();
        chmod_plus_x(&app_dir.join("bin").join("rake")).unwrap();

        fs_err::write(app_dir.join("Rakefile"), r#"
            STDOUT.sync = true
            STDERR.sync = true

            task "assets:precompile" do
              out = String.new
              out << "START RAKE TEST OUTPUT\n"
              out << run!("echo $PATH")
              out << run!("which -a rake")
              out << run!("which -a ruby")
              out << "END RAKE TEST OUTPUT\n"
              puts out
            end

            def run!(cmd)
              output = String.new
              output << "$ #{cmd}\n"
              output << `#{cmd} 2>&1`
              raise "Command #{cmd} failed with output #{output}" unless $?.success?
              output
            end
        "#).unwrap();


        context.rebuild(config, |rebuild_context| {
            println!("{}", rebuild_context.pack_stdout);
            let rake_output = Regex::new(r"(?sm)START RAKE TEST OUTPUT\n(.*)END RAKE TEST OUTPUT").unwrap().captures(&rebuild_context.pack_stdout).and_then(|captures| captures.get(1).map(|m| m.as_str().to_string())).unwrap();
            assert_eq!(
                r"
      $ echo $PATH
      /layers/heroku_ruby/gems/bin:/workspace/bin:/layers/heroku_ruby/bundler/bin:/layers/heroku_ruby/binruby/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
      $ which -a rake
      /layers/heroku_ruby/gems/bin/rake
      /workspace/bin/rake
      /layers/heroku_ruby/binruby/bin/rake
      $ which -a ruby
      /layers/heroku_ruby/binruby/bin/ruby
           ".trim(),
        rake_output.trim()
);

            let command_output = rebuild_context.run_shell_command(
                indoc! {"
                    # Output command + output to stdout
                    export BASH_XTRACEFD=1; set -o xtrace
                    which -a rake
                "}
            );
            assert_empty!(command_output.stderr);
            assert_eq!(
                formatdoc! {"
                    + which -a rake
                    /workspace/bin/rake
                    /layers/heroku_ruby/gems/bin/rake
                    /layers/heroku_ruby/binruby/bin/rake
                "},
                command_output.stdout,
            );
        });
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
                r#"`BUNDLE_FROZEN="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_WITHOUT="development:test" bundle install`"#
            );

            assert_contains!(context.pack_stdout, "Installing puma");

            let secret_key_base = context.run_shell_command("echo \"${SECRET_KEY_BASE:?No SECRET_KEY_BASE set}\"").stdout;
            assert!(!secret_key_base.trim().is_empty(), "Expected {secret_key_base:?} to not be empty but it is");

            let config = context.config.clone();
            context.rebuild(config, |rebuild_context| {
                println!("{}", rebuild_context.pack_stdout);

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
                r#"`BUNDLE_FROZEN="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_WITHOUT="development:test" bundle install`"#
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
            BuildpackReference::Other(String::from("heroku/nodejs")),
            BuildpackReference::CurrentCrate,
        ]),
        |context| {
            println!("{}", context.pack_stdout);
            assert_contains!(context.pack_stdout, "pruning was disabled by a participating buildpack");
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");
            assert_contains!(
                context.pack_stdout,
                r#"`BUNDLE_FROZEN="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_WITHOUT="development:test" bundle install`"#
            );
            }
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

/// Sets file permissions on the given path to 7xx (similar to `chmod +x <path>`)
///
/// i.e. chmod +x will ensure that the first digit
/// of the file permission is 7 on unix so if you pass
/// in 0o455 it would be mutated to 0o755
fn chmod_plus_x(path: &Path) -> Result<(), std::io::Error> {
    let mut perms = fs_err::metadata(path)?.permissions();
    let mut mode = perms.mode();
    mode |= 0o700;
    perms.set_mode(mode);

    fs_err::set_permissions(path, perms)
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<(), std::io::Error> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    fs_err::create_dir_all(dst)?;
    for entry in fs_err::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.join(entry.file_name()))?;
        } else {
            fs_err::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
