use indoc::formatdoc;
use libherokubuildpack::log as user;

use crate::{ruby_version::RubyVersionError, RubyBuildpackError};

pub(crate) fn on_error(err: libcnb::Error<RubyBuildpackError>) {
    match cause(err) {
        Cause::OurError(error) => log_our_error(error),
        Cause::FrameworkError(error) => user::log_error(
            "heroku/buildpack-ruby internal buildpack error",
            formatdoc! {"
                An unexpected internal error was reported by the framework used
                by this buildpack.

                If the issue persists, consider opening an issue on the GitHub
                repository. If you are unable to deploy to Heroku as a result
                of this issue, consider opening a ticket for additional support.

                Details:

                {error}
            "},
        ),
    };
}

#[allow(clippy::too_many_lines)]
fn log_our_error(error: RubyBuildpackError) {
    match error {
        RubyBuildpackError::RubyVersionError(error) => match error {
            RubyVersionError::JRubyMissingRubyVersion(engine_version) => user::log_error(
                "JRuby specified without a Ruby version",
                formatdoc! {"
                We found JRuby version {engine_version} specified
                in your Gemfile.lock, but no associated Ruby version.

                Each JRuby version implements one or more specifications
                of Ruby. The buildpack cannot continue.

                Ensure that the `Gemfile.lock` in the root of your project
                is valid.

                Help: Verify you can run `bundle install` against your application
                before trying again.
                "},
            ),
            RubyVersionError::SystemRubyDetectionFailed(details) => user::log_error(
                "Error validating system ruby",
                formatdoc! {"
                Before continuing the buildpack needs to ensure there is a version
                of ruby on the system. The buildpack checked for a ruby version by
                running a command on the system.

                The check command failed and the buildpack cannot continue:

                {details}

                Help: Verify that a ruby version is being correctly installed
                on the PATH  before trying again.
            "},
            ),
        },
        RubyBuildpackError::RakeDetectError(error) => user::log_error(
            "Error detecting rake tasks",
            format! {"
            The Ruby buildpack uses rake task information from your application to guide
            build logic. Without this information, the Ruby buildpack cannot continue.

            Details:

            {error}

            "},
        ),
        RubyBuildpackError::GemListGetError(error) => user::log_error(
            "Error detecting dependencies",
            format! {"
            The Ruby buildpack uses dependency information from your application to
            guide build logic. Without this information, the Ruby buildpack cannot
            continue.

            Details:

            {error}
            "},
        ),
        RubyBuildpackError::RubyInstallError(error) => user::log_error(
            "Error installing Ruby",
            format! {"
            Could not install the detected Ruby version.

            Details:

            {error}
            "},
        ),
        RubyBuildpackError::MissingGemfileLock(error) => user::log_error(
            "Error: Gemfile.lock required",
            format! {"
            To deploy a Ruby application, a Gemfile.lock file is required in the
            root of your application, but none was found.

            If you have a Gemfile.lock in your application, you may not have it
            tracked in git, or you may be on a different branch.

            Details:

            {error}
            "},
        ),
        RubyBuildpackError::InAppDirCacheError(error) => user::log_error(
            "Internal cache error",
            format! {"
            An internal error occured while caching files.

            Details:

            {error}
            "},
        ),
        RubyBuildpackError::BundleInstallDigestError(error) => user::log_error(
            "Could not generate digest",
            format! {"
            To provide the fastest possible install experience the Ruby buildpack
            converts Gemfile and Gemfile.lock into a cryptographic digest to be
            used in cache invalidation.

            While performing this process there was an unexpected internal error.

            Details:
            {error}
            "},
        ),
        RubyBuildpackError::BundleInstallCommandError(error) => user::log_error(
            "Error installing bundler",
            format! {"
            Installation of bundler failed. Bundler is the package managment
            library for Ruby. Bundler is needed to install your application's dependencies
            listed in the Gemfile.

            Command failed:

            {error}
            "},
        ),
        RubyBuildpackError::RakeAssetsPrecompileFailed(error) => user::log_error(
            "Asset compilation failed",
            format! {"
            An error occured while compiling assets via rake command.

            Command failed:

            {error}
            "},
        ),
        RubyBuildpackError::GemInstallBundlerCommandError(error) => user::log_error(
            "Installing gems failed",
            format! {"
            Could not install gems to the system via bundler. Gems are dependencies
            your application listed in the Gemfile and resolved in the Gemfile.lock.

            Command failed:

            {error}
            "},
        ),
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
