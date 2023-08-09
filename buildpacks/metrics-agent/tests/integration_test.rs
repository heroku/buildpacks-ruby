#![warn(clippy::pedantic)]

use indoc::formatdoc;
use libcnb_test::{
    assert_contains, assert_empty, BuildConfig, BuildpackReference, ContainerConfig,
    ContainerContext, TestRunner,
};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use thiserror::__private::DisplayAsDisplay;
use ureq::Response;

#[test]
#[ignore = "integration test"]
fn test_barnes_app() {
    TestRunner::default().build(
        BuildConfig::new("heroku/builder:22", "tests/fixtures/barnes_app").buildpacks(vec![
            BuildpackReference::Crate,
            BuildpackReference::Local(PathBuf::from("../ruby")),
        ]),
        |context| {
            assert_contains!(context.pack_stdout, "# Heroku StatsD Metrics Agent");
            assert_contains!(context.pack_stdout, "# Heroku Ruby Buildpack");

            context.start_container(
                ContainerConfig::new()
                    .entrypoint(["launcher"])
                    .envs(vec![
                        ("HEROKU_METRICS_URL", "example.com"),
                        ("DYNO", "web.1"),
                    ])
                    .command(["ps x"]),
                |container| {
                    let log_output = container.logs_wait();
                    assert_contains!(log_output.stdout, "agentmon_loop --path");
                },
            );
        },
    );
}
