use commons::{
    cache::CacheError,
    fun_run::{CmdError, CmdErrorDiagnostics, ErrorDiagnostics, NamedOutput},
    metadata_digest::DigestError,
};
use heroku_ruby_buildpack::user_errors::{log_ruby_error, RubyBuildpackError, RubyInstallError};
use indoc::formatdoc;
use std::{
    os::unix::process::ExitStatusExt,
    process::{ExitStatus, Output},
};
use tempfile::tempdir;

fn main() {
    log_error_variants();
}

fn log_error_variants() {
    println!(
        "{}",
        formatdoc! {"

            # Printing Ruby errors

            The error data is fabricated and may diverge from main.rs.
            Use this output to adjust and inspect formatting.

        "}
    );

    let error = RubyBuildpackError::CannotDetectRakeTasks(rake_dash_p_error());
    print_error(error);
    let error = RubyBuildpackError::BundleListError(bundle_list_error());
    print_error(error);
    let error = RubyBuildpackError::RubyInstallError(ruby_install_error());
    print_error(error);
    let error = RubyBuildpackError::MissingGemfileLock(missing_gemfile());
    print_error(error);
    let error = RubyBuildpackError::RakeAssetsCacheError(cache_assets_error());
    print_error(error);
    let error = RubyBuildpackError::BundleInstallDigestError(bundle_digest_error());
    print_error(error);
    let error = RubyBuildpackError::BundleInstallCommandError(bundle_install_user_error());
    print_error(error);
    let error = RubyBuildpackError::RakeAssetsPrecompileFailed(rake_assets_precompile_fail());
    print_error(error);
    let error = RubyBuildpackError::GemInstallBundlerCommandError(gem_install_bundler_error());
    print_error(error);
}

fn gem_install_bundler_error() -> CmdError {
    fake_stdio_command_error("gem install bundler", "bash: command not found: gem")
}

fn rake_assets_precompile_fail() -> CmdError {
    fake_stream_command_error(
        r#"RAILS_ENV=production rake assets:precompile assets:clean --trace"#,
        formatdoc! {"
        Failed while compiling assets
        but also since this error is already
        streamed
        I shouldn't ever show up in the output'
    "},
    )
}

fn bundle_install_user_error() -> CmdError {
    fake_stream_command_error(
        r#"BUNDLE_DEPLOYMENT=1 BUNDLE_WITHOUT="development:test" bundle install"#,
        formatdoc! {"
        Could not fetch gem 'i do not exist,
        but also since this error is already
        streamed
        I shouldn't ever show up in the output'
    "},
    )
}

fn bundle_digest_error() -> DigestError {
    DigestError::CannotReadFile(cannot_read_file())
}

fn cache_assets_error() -> CacheError {
    CacheError::CachedPathNotInAppPath("/workspace/public/assets".to_string())
}

fn cannot_read_file() -> std::io::Error {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Gemfile.lock");
    let result = fs_err::read(&path);

    let error = match result {
        Ok(_) => panic!("{} should not exist", path.display()),
        Err(e) => e,
    };
    error
}

fn missing_gemfile() -> ErrorDiagnostics<std::io::Error> {
    ErrorDiagnostics::new(cannot_read_file())
}

fn ruby_install_error() -> RubyInstallError {
    let uri = String::from(
        "https://heroku-buildpack-ruby.s3.us-east-1.amazonaws.com/heroku-22/ruby-3.0.lol.tgz",
    );
    let response = ureq::get(uri.as_ref())
        .call()
        .map_err(|err| RubyInstallError::RequestError(Box::new(err)));

    match response {
        Ok(_) => panic!("path should not exist"),
        Err(e) => e,
    }
}

fn bundle_list_error() -> CmdErrorDiagnostics {
    let error = fake_stdio_command_error("bundle list", "bash: command not found: bundle");
    CmdErrorDiagnostics::new(error)
}

fn rake_dash_p_error() -> CmdError {
    fake_nonzero_command_error(
        "bundle exec rake -P --trace",
        formatdoc! {"
      rake aborted!
      LoadError: cannot load such file -- does_not_exist
      /workspace/Rakefile:1:in `require'
      /workspace/Rakefile:1:in `<top (required)>'
      /layers/heroku_ruby/gems/ruby/3.1.0/gems/rake-13.0.6/lib/rake/rake_module.rb:29:in `load'
      /layers/heroku_ruby/gems/ruby/3.1.0/gems/rake-13.0.6/lib/rake/rake_module.rb:29:in `load_rakefile'
      /layers/heroku_ruby/gems/ruby/3.1.0/gems/rake-13.0.6/lib/rake/application.rb:710:in `raw_load_rakefile'
      /layers/heroku_ruby/gems/ruby/3.1.0/gems/rake-13.0.6/lib/rake/application.rb:104:in `block in load_rakefile'
      /layers/heroku_ruby/gems/ruby/3.1.0/gems/rake-13.0.6/lib/rake/application.rb:186:in `standard_exception_handling'
      /layers/heroku_ruby/gems/ruby/3.1.0/gems/rake-13.0.6/lib/rake/application.rb:103:in `load_rakefile'
      /layers/heroku_ruby/gems/ruby/3.1.0/gems/rake-13.0.6/lib/rake/application.rb:82:in `block in run'
      /layers/heroku_ruby/gems/ruby/3.1.0/gems/rake-13.0.6/lib/rake/application.rb:186:in `standard_exception_handling'
      /layers/heroku_ruby/gems/ruby/3.1.0/gems/rake-13.0.6/lib/rake/application.rb:80:in `run'
      /layers/heroku_ruby/gems/ruby/3.1.0/gems/rake-13.0.6/exe/rake:27:in `<top (required)>'
      /layers/heroku_ruby/gems/ruby/3.1.0/bin/rake:25:in `load'
      /layers/heroku_ruby/gems/ruby/3.1.0/bin/rake:25:in `<top (required)>'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/cli/exec.rb:58:in `load'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/cli/exec.rb:58:in `kernel_load'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/cli/exec.rb:23:in `run'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/cli.rb:492:in `exec'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/vendor/thor/lib/thor/command.rb:27:in `run'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/vendor/thor/lib/thor/invocation.rb:127:in `invoke_command'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/vendor/thor/lib/thor.rb:392:in `dispatch'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/cli.rb:34:in `dispatch'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/vendor/thor/lib/thor/base.rb:485:in `start'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/cli.rb:28:in `start'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/exe/bundle:37:in `block in <top (required)>'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/lib/bundler/friendly_errors.rb:117:in `with_friendly_errors'
      /layers/heroku_ruby/bundler/gems/bundler-2.4.15/exe/bundle:29:in `<top (required)>'
      /layers/heroku_ruby/gems/bin/bundle:108:in `load'
      /layers/heroku_ruby/gems/bin/bundle:108:in `<main>'
    "},
    )
}

fn fake_stream_command_error(name: impl AsRef<str>, error: impl AsRef<str>) -> CmdError {
    let name = name.as_ref().to_string();
    let error = error.as_ref().to_string().into_bytes();

    let status = ExitStatus::from_raw(1);
    CmdError::NonZeroExitAlreadyStreamed(NamedOutput {
        name,
        output: Output {
            status,
            stdout: Vec::new(),
            stderr: error,
        },
    })
}

fn fake_nonzero_command_error(name: impl AsRef<str>, error: impl AsRef<str>) -> CmdError {
    let name = name.as_ref().to_string();
    let error = error.as_ref().to_string().into_bytes();

    let status = ExitStatus::from_raw(1);
    CmdError::NonZeroExitNotStreamed(NamedOutput {
        name,
        output: Output {
            status,
            stdout: Vec::new(),
            stderr: error,
        },
    })
}

fn fake_stdio_command_error(name: impl AsRef<str>, error: impl AsRef<str>) -> CmdError {
    let name = name.as_ref().to_string();
    let error = error.as_ref().to_string();

    let error = std::io::Error::new(std::io::ErrorKind::NotFound, error);
    CmdError::SystemError(name, error)
}

fn print_error(error: RubyBuildpackError) {
    let name = format!("{error:?}");

    // Grab part of the debug output before the first parens
    let re = regex::Regex::new("([^\\(]*)").expect("clippy");
    let name = &re.captures(&name).unwrap()[0];

    println!("## Error message for RubyBuildpackError::{name}");
    println!();
    println!("```");
    log_ruby_error(error);
    println!("```");
    println!();
}
