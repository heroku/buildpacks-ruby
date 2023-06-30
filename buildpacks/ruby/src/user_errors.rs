use indoc::formatdoc;

use crate::{build_output::fmt::ErrorInfo, RubyBuildpackError};

pub(crate) fn on_error(err: libcnb::Error<RubyBuildpackError>) {
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

fn log_our_error(error: RubyBuildpackError) {
    match error {
        RubyBuildpackError::RakeDetectError(error) => ErrorInfo::header_body_details(
            "Error detecting rake tasks",
            formatdoc! {"
            The Ruby buildpack uses rake task information from your application to guide
            build logic. Without this information, the Ruby buildpack cannot continue.
            "},
            error,
        )
        .print(),
        RubyBuildpackError::GemListGetError(error) => ErrorInfo::header_body_details(
            "Error detecting dependencies",
            formatdoc! {"
            The Ruby buildpack uses dependency information from your application to
            guide build logic. Without this information, the Ruby buildpack cannot
            continue.
            "},
            error,
        )
        .print(),
        RubyBuildpackError::RubyInstallError(error) => ErrorInfo::header_body_details(
            "Error installing Ruby",
            formatdoc! {"
            Could not install the detected Ruby version.
            "},
            error,
        )
        .print(),
        RubyBuildpackError::MissingGemfileLock(error) => ErrorInfo::header_body_details(
            "Error: Gemfile.lock required",
            formatdoc! {"
            To deploy a Ruby application, a Gemfile.lock file is required in the
            root of your application, but none was found.

            If you have a Gemfile.lock in your application, you may not have it
            tracked in git, or you may be on a different branch.
            "},
            error,
        )
        .print(),
        RubyBuildpackError::InAppDirCacheError(error) => ErrorInfo::header_body_details(
            "Internal cache error",
            formatdoc! {"
            An internal error occured while caching files.
            "},
            error,
        )
        .print(),
        RubyBuildpackError::BundleInstallDigestError(error) => ErrorInfo::header_body_details(
            "Could not generate digest",
            formatdoc! {"
            To provide the fastest possible install experience the Ruby buildpack
            converts Gemfile and Gemfile.lock into a cryptographic digest to be
            used in cache invalidation.

            While performing this process there was an unexpected internal error.
            "},
            error,
        )
        .print(),
        RubyBuildpackError::BundleInstallCommandError(error) => ErrorInfo::header_body_details(
            "Error installing bundler",
            formatdoc! {"
            Installation of bundler failed. Bundler is the package managment
            library for Ruby. Bundler is needed to install your application's dependencies
            listed in the Gemfile.
            "},
            error,
        )
        .print(),
        RubyBuildpackError::RakeAssetsPrecompileFailed(error) => ErrorInfo::header_body_details(
            "Asset compilation failed",
            formatdoc! {"
            An error occured while compiling assets via rake command.
            "},
            error,
        )
        .print(),
        RubyBuildpackError::GemInstallBundlerCommandError(error) => ErrorInfo::header_body_details(
            "Installing gems failed",
            formatdoc! {"
            Could not install gems to the system via bundler. Gems are dependencies
            your application listed in the Gemfile and resolved in the Gemfile.lock.
            "},
            error,
        )
        .print(),
    }
}

#[derive(Debug)]
enum Cause {
    OurError(RubyBuildpackError),
    FrameworkError(libcnb::Error<RubyBuildpackError>),
}

fn cause(err: libcnb::Error<RubyBuildpackError>) -> Cause {
    match err {
        libcnb::Error::BuildpackError(err) => Cause::OurError(err),
        err => Cause::FrameworkError(err),
    }
}
