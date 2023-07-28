use crate::{
    build_output::{self, fmt::ErrorInfo},
    RubyBuildpackError,
};
use indoc::formatdoc;

pub(crate) fn on_error(err: libcnb::Error<RubyBuildpackError>) {
    match cause(err) {
        Cause::OurError(error) => log_our_error(error),
        Cause::FrameworkError(error) => ErrorInfo::header_body_details(
            "heroku/buildpack-ruby internal buildpack error",
            formatdoc! {"
                The framework used by this buildpack encountered an unexpected error.
                This type of error usually indicates there's nothing wrong with your application.

                If you can’t deploy to Heroku due to this issue please check the official Heroku
                status page https://status.heroku.com/ to see if there is an ongoing incident. Once
                all incidents have resolved please retry your build.

                If the issue persists, please try to reproduce the behavior locally using the `pack`
                CLI. If you can reproduce the behavior locally and believe you've found a bug in the
                buildpack or the framework please open an issue on the buildpack's GitHub repository.
            "},
            error,
        )
        .print(),
    };
}

#[allow(clippy::too_many_lines)]
fn log_our_error(error: RubyBuildpackError) {
    let file_hints = file_hints();
    match error {
        RubyBuildpackError::CannotDetectRakeTasks(error) => ErrorInfo::header_body_details(
            "Error detecting rake tasks",
            formatdoc! {"
            The Ruby buildpack uses rake task information from your application to guide
            build logic. Without this information, the Ruby buildpack cannot continue.

            Try to reproduce the error locally by running the command below.
            Once you've fixed all errors locally, commit the result to git and retry
            your build.

            If your build continues to fail, application requirements, such as provisioned add-ons,
            environment variables, or installed system packages may be needed. Use the
            information below to debug further.
            "},
            error,
        )
        .print(),
        RubyBuildpackError::BundleListError(error) => ErrorInfo::header_body_details(
            "Error detecting dependencies",
            formatdoc! {"
            The Ruby buildpack requires information about your application’s dependencies to
            complete the build. Without this information, the Ruby buildpack cannot continue.

            Use the following information to help debug the system.
            "},
            error,
        )
        .print(),
        RubyBuildpackError::RubyInstallError(error) => ErrorInfo {
            header: "Error installing Ruby".to_string(),
            body: formatdoc! {"
                Could not install the detected Ruby version. Ensure that you're using a supported
                ruby version and try again.
            "},
            url: build_output::fmt::Url::Label {
                label: "Supported ruby versions".to_string(),
                url: "https://devcenter.heroku.com/articles/ruby-support#ruby-versions".to_string(),
            },
            debug_details: Some(error.to_string()),
        }
        .print(),
        RubyBuildpackError::MissingGemfileLock(error) => ErrorInfo {
            header: "Gemfile.lock` not found".to_string(),
            body: formatdoc! {"
                A `Gemfile.lock` file is required and was not found in the root of your application.

                If you have a `Gemfile.lock` in your application, ensure it’s tracked in Git and
                that you’re pushing the correct branch.
            "},
            url: build_output::fmt::Url::MoreInfo(
                "https://devcenter.heroku.com/articles/git#deploy-from-a-branch-besides-main"
                    .to_string(),
            ),
            debug_details: Some(error.to_string()),
        }
        .print(),
        RubyBuildpackError::RakeAssetsCacheError(error) => ErrorInfo::header_body_details(
            "Error caching frontend assets",
            formatdoc! {"
                An error occurred while attempting to cache frontend assets, and the Ruby buildpack cannot continue.

                {file_hints}
            "},
            error,
        )
        .print(),
        RubyBuildpackError::BundleInstallDigestError(error) => ErrorInfo::header_body_details(
            "Failed to generate file digest",
            formatdoc! {"
                An error occurred while generating a file digest. To provide the fastest possible install experience,
                the Ruby buildpack converts your `Gemfile` and `Gemfile.lock` into a digest to use in cache invalidation.

                {file_hints}

                If you're unable to resolve this error, you can disable the the digest feature by setting the environment variable:

                HEROKU_SKIP_BUNDLE_DIGEST=1
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

fn file_hints() -> String {
    formatdoc! {"
        Ensure that the permissions on the files in your application directory are correct and that
        all symlinks correctly resolve.

        If you believe that your application is correct, ensure all files are tracked in Git and
        that you’re pushing the correct branch:

        https://devcenter.heroku.com/articles/git#deploy-from-a-branch-besides-main

        If this failure is occuring while deploying to Heroku check the status page https://status.heroku.com/
        for incidents. Once all incidents have been resolved, please retry your build.
    "}
}
