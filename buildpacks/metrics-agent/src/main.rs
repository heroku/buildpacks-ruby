mod layers;

use crate::layers::download_agentmon::{DownloadAgentmon, DownloadAgentmonError};
use crate::layers::spawn_agemntmon_execd::SpawnAgentmonExecd;
use commons::build_output::{self, fmt::ErrorInfo};
use indoc::formatdoc;
use libcnb::{
    build::{BuildContext, BuildResult, BuildResultBuilder},
    buildpack_main,
    data::{build_plan::BuildPlanBuilder, layer_name},
    detect::{DetectContext, DetectResult, DetectResultBuilder},
    Buildpack,
};

#[derive(Debug)]
enum MetricsAgentError {
    DownloadAgentmonError(DownloadAgentmonError),
}

impl From<MetricsAgentError> for libcnb::Error<MetricsAgentError> {
    fn from(error: MetricsAgentError) -> Self {
        libcnb::Error::BuildpackError(error)
    }
}

buildpack_main!(MetricsAgentBuildpack);

pub(crate) struct MetricsAgentBuildpack;

impl Buildpack for MetricsAgentBuildpack {
    type Platform = libcnb::generic::GenericPlatform;
    type Metadata = libcnb::generic::GenericMetadata;
    type Error = MetricsAgentError;

    fn detect(&self, context: DetectContext<Self>) -> libcnb::Result<DetectResult, Self::Error> {
        let plan_builder = BuildPlanBuilder::new().provides("heroku-statsd-metrics-agent");

        if let Ok(true) = fs_err::read_to_string(context.app_dir.join("Gemfile.lock"))
            .map(|lockfile| lockfile.contains("barnes"))
        {
            DetectResultBuilder::pass()
                .build_plan(plan_builder.requires("heroku-statsd-metrics-agent").build())
                .build()
        } else {
            DetectResultBuilder::pass()
                .build_plan(plan_builder.build())
                .build()
        }
    }

    fn build(&self, context: BuildContext<Self>) -> libcnb::Result<BuildResult, Self::Error> {
        let build_duration = build_output::buildpack_name("Heroku Statsd Metrics Agent");

        // let section = build_output::section("Metrics agent");
        // context.handle_layer(
        //     layer_name!("statsd-metrics-agent"),
        //     DownloadAgentmon { section },
        // )?;
        context.handle_layer(layer_name!("layer_name"), SpawnAgentmonExecd)?;

        // TODO write launch script
        //
        // if [[ "${AGENTMON_DEBUG}" = "true" ]]; then
        //     AGENTMON_FLAGS+=("-debug")
        // fi

        // if [[ -x "${BUILD_DIR}/bin/agentmon" ]]; then
        //     (while true; do
        //         ${BUILD_DIR}/bin/agentmon "${AGENTMON_FLAGS[@]}" "${HEROKU_METRICS_URL}"
        //         echo "agentmon completed with status=${?}. Restarting"
        //         sleep 1
        //     done) &
        // else
        //     echo "No agentmon executable found. Not starting."
        // fi

        build_duration.done_timed();
        BuildResultBuilder::new().build()
    }

    fn on_error(&self, err: libcnb::Error<Self::Error>) {
        on_error(err);
    }
}

#[derive(Debug)]
enum Cause {
    OurError(MetricsAgentError),
    FrameworkError(libcnb::Error<MetricsAgentError>),
}

fn cause(err: libcnb::Error<MetricsAgentError>) -> Cause {
    match err {
        libcnb::Error::BuildpackError(err) => Cause::OurError(err),
        err => Cause::FrameworkError(err),
    }
}

pub(crate) fn on_error(err: libcnb::Error<MetricsAgentError>) {
    match cause(err) {
        Cause::OurError(error) => log_our_error(error),
        Cause::FrameworkError(error) => ErrorInfo::header_body_details(
            "heroku/buildpack-ruby internal buildpack error",
            formatdoc! {"
                An unexpected internal error was reported by the framework used
                by this buildpack.

                If the issue persists, consider opening an issue on the GitHub
                repository. If you are unable to deploy to Heroku as a result
                of this issue, consider opening a ticket for additional support.
            "},
            error,
        )
        .print(),
    };
}

fn log_our_error(error: MetricsAgentError) {
    match error {
        MetricsAgentError::DownloadAgentmonError(error) => ErrorInfo::header_body_details(
            formatdoc! {
                "Could not install Statsd agent"
            },
            formatdoc! {
                "An error occured while downloading and installing the metrics agent
                the buildpack cannot continue"
            },
            error,
        )
        .print(),
    }
}
