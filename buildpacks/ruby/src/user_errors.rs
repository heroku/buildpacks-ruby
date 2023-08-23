#[allow(clippy::wildcard_imports)]
use commons::output::{interface::*, log::BuildLog};

use crate::RubyBuildpackError;
use indoc::formatdoc;

pub(crate) fn on_error(err: libcnb::Error<RubyBuildpackError>) {
    let mut log = BuildLog::new(std::io::stdout());
    match cause(err) {
        Cause::OurError(error) => log_our_error(error),
        Cause::FrameworkError(_error) => log.error(&formatdoc! {"
                Error: heroku/buildpack-ruby internal buildpack error

                An unexpected internal error was reported by the framework used
                by this buildpack.

                If the issue persists, consider opening an issue on the GitHub
                repository. If you are unable to deploy to Heroku as a result
                of this issue, consider opening a ticket for additional support.
            "}),
    };
}

fn log_our_error(error: RubyBuildpackError) {
    let mut log = BuildLog::new(std::io::stdout());
    match error {
        RubyBuildpackError::RakeDetectError(_error) => log.error(&formatdoc! {"
            Error detecting rake tasks

            The Ruby buildpack uses rake task information from your application to guide
            build logic. Without this information, the Ruby buildpack cannot continue.
            "}),

        RubyBuildpackError::GemListGetError(_error) => log.error(&formatdoc! {"
            Error detecting dependencies

            The Ruby buildpack uses dependency information from your application to
            guide build logic. Without this information, the Ruby buildpack cannot
            continue.
            "}),

        RubyBuildpackError::RubyInstallError(_error) => log.error(&formatdoc! {"
            Error installing Ruby

            Could not install the detected Ruby version.
            "}),
        RubyBuildpackError::MissingGemfileLock(_error) => log.error(&formatdoc! {"
            Error: Gemfile.lock required

            To deploy a Ruby application, a Gemfile.lock file is required in the
            root of your application, but none was found.

            If you have a Gemfile.lock in your application, you may not have it
            tracked in git, or you may be on a different branch.
            "}),
        RubyBuildpackError::InAppDirCacheError(_error) => log.error(&formatdoc! {"
            Internal cache error

            An internal error occured while caching files.
            "}),
        RubyBuildpackError::BundleInstallDigestError(_error) => log.error(&formatdoc! {"
            Could not generate digest

            To provide the fastest possible install experience the Ruby buildpack
            converts Gemfile and Gemfile.lock into a cryptographic digest to be
            used in cache invalidation.

            While performing this process there was an unexpected internal error.
            "}),
        RubyBuildpackError::BundleInstallCommandError(_error) => log.error(&formatdoc! {"
            Error installing bundler

            Installation of bundler failed. Bundler is the package managment
            library for Ruby. Bundler is needed to install your application's dependencies
            listed in the Gemfile.
            "}),
        RubyBuildpackError::RakeAssetsPrecompileFailed(_error) => log.error(&formatdoc! {"
            Asset compilation failed

            An error occured while compiling assets via rake command.
            "}),
        RubyBuildpackError::GemInstallBundlerCommandError(_error) => log.error(&formatdoc! {"
            Installing gems failed

            Could not install gems to the system via bundler. Gems are dependencies
            your application listed in the Gemfile and resolved in the Gemfile.lock.
            "}),
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
