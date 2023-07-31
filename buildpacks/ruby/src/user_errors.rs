use crate::RubyBuildpackError;
use commons::{
    build_output::{self, paragraph::ErrorBuilder},
    fun_run::CmdError,
};
use indoc::formatdoc;

pub(crate) fn on_error(err: libcnb::Error<RubyBuildpackError>) {
    match cause(err) {
        Cause::OurError(error) => log_our_error(error),
        Cause::FrameworkError(error) => ErrorBuilder::new(
            "heroku/buildpack-ruby internal buildpack error"
            ).body(formatdoc! {"
                The framework used by this buildpack encountered an unexpected error.
                This type of error usually indicates there's nothing wrong with your application.
            "})
            .body(formatdoc! {"
                If you can’t deploy to Heroku due to this issue please check the official Heroku
                status page https://status.heroku.com/ to see if there is an ongoing incident. Once
                all incidents have resolved please retry your build.
            "})
            .body(formatdoc! {"
                If the issue persists, please try to reproduce the behavior locally using the `pack`
                CLI. If you can reproduce the behavior locally and believe you've found a bug in the
                buildpack or the framework please open an issue on the buildpack's GitHub repository.
            "})
            .debug_details(&error)
            .print(),
    };
}

#[allow(clippy::too_many_lines)]
fn log_our_error(error: RubyBuildpackError) {
    let file_hints = file_hints();
    let git_branch = git_branch();
    let heroku_status = heroku_status();

    match error {
        RubyBuildpackError::CannotDetectRakeTasks(error) => {
            let cmd_debug = match &error {
                crate::rake_task_detect::CannotDetectRakeTasks::DashpCommandError(error) => local_command_debug(error),
            };
            ErrorBuilder::new(
                "Error detecting rake tasks"
                )
                .body(formatdoc! {"
                    The Ruby buildpack uses rake task information from your application to guide
                    build logic. Without this information, the Ruby buildpack cannot continue.
                "})
                .body(cmd_debug)
                .body(formatdoc! {"
                    If your build continues to fail, application requirements, such as provisioned add-ons,
                    environment variables, or installed system packages may be needed. Use the
                    information below to debug further.
                "})
                .debug_details(&error)
                .print();
        },
        RubyBuildpackError::BundleListError(error) => ErrorBuilder::new(
            "Error detecting dependencies"
            )
            .body(formatdoc! {"
                The Ruby buildpack requires information about your application’s dependencies to
                complete the build. Without this information, the Ruby buildpack cannot continue.

                Use the following information to help debug the system.
            "})
            .debug_details(&error)
            .print(),
        RubyBuildpackError::RubyInstallError(error) => ErrorBuilder::new(
             "Error installing Ruby",
            ).body(formatdoc! {"
                Could not install the detected Ruby version. Ensure that you're using a supported
                ruby version and try again.
            "})
            .url(build_output::paragraph::Url::Label {
                label: "Supported ruby versions".to_string(),
                url: "https://devcenter.heroku.com/articles/ruby-support#ruby-versions".to_string(),
            })
            .debug_details(&error)
            .print(),
        RubyBuildpackError::MissingGemfileLock(error) => ErrorBuilder::new(
            "Gemfile.lock` not found"
            )
            .body(formatdoc! {"
                A `Gemfile.lock` file is required and was not found in the root of your application.

                If you have a `Gemfile.lock` in your application, ensure it is tracked in Git and
                that you’re pushing the correct branch.
            "})
            .url(build_output::paragraph::Url::MoreInfo(
                "https://devcenter.heroku.com/articles/git#deploy-from-a-branch-besides-main"
                    .to_string(),
            )).
            debug_details(&error)
            .print(),
        RubyBuildpackError::RakeAssetsCacheError(error) => ErrorBuilder::new(
            "Error caching frontend assets"
            )
            .body(formatdoc! {"
                An error occurred while attempting to cache frontend assets, and the Ruby buildpack cannot continue.
            "})
            .body(file_hints)
            .debug_details(&error)
            .print(),
        RubyBuildpackError::BundleInstallDigestError(error) => ErrorBuilder::new(
            "Failed to generate file digest"
            )
            .body(formatdoc! {"
                An error occurred while generating a file digest. To provide the fastest possible install experience,
                the Ruby buildpack converts your `Gemfile` and `Gemfile.lock` into a digest to use in cache invalidation.
            "})
            .body(file_hints)
            .body(formatdoc! {"
                If you're unable to resolve this error, you can disable the the digest feature by setting the environment variable:

                HEROKU_SKIP_BUNDLE_DIGEST=1
            "})
            .debug_details(&error)
            .print(),
        RubyBuildpackError::BundleInstallCommandError(error) => ErrorBuilder::new(
            "Failed to install bundler"
            )
            .body(formatdoc! {"
                The ruby package managment tool, `bundler`, failed to install. Bundler is required to install your application's dependencies listed in the `Gemfile`.
            "})
            .body(heroku_status)
            .debug_details(&error)
            .print(),
        RubyBuildpackError::RakeAssetsPrecompileFailed(error) => {
            let cmd_debug = local_command_debug(&error);

            ErrorBuilder::new("Failed to compile assets")
                .body(formatdoc! {"
                    An error occured while compiling assets via rake command. Details of the error are
                    listed below.
                "})
                .body(cmd_debug)
                .body(git_branch)
                .debug_details(&error)
                .print();
        },
        RubyBuildpackError::GemInstallBundlerCommandError(error) => {
            let cmd_debug = local_command_debug(&error);

            ErrorBuilder::new("Failed to install gems")
                .body(formatdoc! {"
                    Could not install gems to the system via bundler. Gems are dependencies
                    your application listed in the Gemfile and resolved in the Gemfile.lock.
                "})
                .body(cmd_debug)
                .body(git_branch)
                .debug_details(&error)
                .print();
        },
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
    let heroku_status = heroku_status();
    formatdoc! {"
        Ensure that the permissions on the files in your application directory are correct and that
        all symlinks correctly resolve.

        {heroku_status}
    "}
}

fn git_branch() -> String {
    let url = build_output::fmt::url(
        "https://devcenter.heroku.com/articles/git#deploy-from-a-branch-besides-main",
    );
    formatdoc! {"
        If you believe that your application is correct, ensure all files are tracked in Git and
        that you’re pushing the correct branch:

        {url}
    "}
}

fn heroku_status() -> String {
    let url = build_output::fmt::url("https://status.heroku.com/");
    formatdoc! {"
        If this failure is occuring while deploying to Heroku check the status page {url}
        for incidents. Once all incidents have been resolved, please retry your build.
    "}
}

fn local_command_debug(error: &CmdError) -> String {
    let cmd_name = replace_app_path_with_relative(build_output::fmt::command(error.name()));

    formatdoc! {"
        Ensure you can run the following command  locally with no errors before attempting another build:

        `{cmd_name}`

    "}
}

fn replace_app_path_with_relative(contents: impl AsRef<str>) -> String {
    let app_path_re = regex::Regex::new("/workspace/").expect("Internal error: regex");

    app_path_re.replace_all(contents.as_ref(), "./").to_string()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_relative_path() {
        let expected = r#"BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="./Gemfile" BUNDLE_WITHOUT="development:test" bundle install"#;
        let actual = replace_app_path_with_relative(
            r#"BUNDLE_DEPLOYMENT="1" BUNDLE_GEMFILE="/workspace/Gemfile" BUNDLE_WITHOUT="development:test" bundle install"#,
        );
        assert_eq!(expected, &actual);
    }
}
