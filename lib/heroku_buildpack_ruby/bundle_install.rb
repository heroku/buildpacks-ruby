require 'benchmark'

module HerokuBuildpackRuby
  class BundleInstall
    BUNDLE_BIN_ENV = EnvProxy.value("BUNDLE_BIN")
    BUNDLE_PATH_ENV = EnvProxy.value("BUNDLE_PATH")
    BUNDLE_WITHOUT_ENV = EnvProxy.default("BUNDLE_WITHOUT")
    BUNDLE_DEPLOYMENT_ENV = EnvProxy.value("BUNDLE_DEPLOYMENT")
    BUNDLE_GLOBAL_PATH_APPENDS_RUBY_SCOPE_ENV = EnvProxy.value("BUNDLE_GLOBAL_PATH_APPENDS_RUBY_SOURCE")

    private; attr_reader :bundle_output, :bundle_without_default, :bundle_install_gems_dir, :user_comms, :bundle_gems_binsub_dir; public

    def initialize(app_dir: , bundle_without_default: , bundle_install_gems_dir:, user_comms: , metadata: Metadata::Null.new)
      @user_comms = user_comms
      @app_dir = Pathname(app_dir)
      @metadata = metadata
      @bundle_without_default = bundle_without_default
      @bundle_install_gems_dir = Pathname(bundle_install_gems_dir)
      @bundle_gems_binsub_dir = @bundle_install_gems_dir.join("bin")
      @bundle_output = nil
    end

    def call
      prepare_env
      bundle_install
      bundle_clean
    end

    private def bundle_install
      bundle_command = String.new("")
      bundle_command << BUNDLE_WITHOUT_ENV.to_env
      bundle_command << BUNDLE_PATH_ENV.to_env
      bundle_command << BUNDLE_BIN_ENV.to_env
      bundle_command << BUNDLE_DEPLOYMENT_ENV.to_env
      bundle_command << "bundle install -j4 --no-clean"

      user_comms.puts "Running: #{bundle_command}"
      time = Benchmark.realtime do
        @bundle_output = Bash.new(bundle_command).stream do |lines|
          user_comms.puts lines
        end
      end

      bundle_install_fail unless $?.success?

      user_comms.puts "Bundle completed (#{"%.2f" % time }s)"
    end

    private def prepare_env
      GEM_PATH_ENV.prepend(
        gems: bundle_install_gems_dir,
        bundler: bundle_install_gems_dir
      )

      BUNDLE_GEMFILE_ENV.set(
        bundler: @app_dir.join("Gemfile").to_s
      )
      BUNDLE_GLOBAL_PATH_APPENDS_RUBY_SCOPE_ENV.set(bundler: 1)

      BUNDLE_WITHOUT_ENV.set_default(bundler: bundle_without_default)

      fix_bundle_without_space if BUNDLE_WITHOUT_ENV.value.include?(' ')

      BUNDLE_PATH_ENV.set(bundler: bundle_install_gems_dir)
      BUNDLE_BIN_ENV.set(bundler: bundle_gems_binsub_dir)
      BUNDLE_DEPLOYMENT_ENV.set(bundler: 1)
    end

    private def bundle_clean
      if bundle_output.match?(/Fetching/)
        user_comms.puts "Cleaning up the bundler cache"

        Bash.new("bundle clean").stream do |lines|
          user_comms.puts lines
        end
      else
        user_comms.puts "Skipping cleaning bundler cache (no new gems detected)"
      end
    end

    private def bundle_failed
      message = String.new("Failed to install gems via Bundler.")
      message << sqlite_error_message if bundler_output.match(/An error occurred while installing sqlite3/)
      message << gemfile_ruby_version_error if bundler_output.match(/but your Gemfile specified/)
      user_comms.error_and_exit(message)
    end


    private def fix_bundle_without_space
      BUNDLE_WITHOUT_ENV.set_without_record(BUNDLE_WITHOUT_ENV.value.tr(" ", ":"))

      message = <<~EOM
        Your BUNDLE_WITHOUT contains a space, it should be a colon `:`
        We have temporarilly set it for your `bundle install` command.

        We recommend updating your application configuration:

        $ heroku config:set BUNDLE_WITHOUT="#{BUNDLE_WITHOUT_ENV.value}"

      EOM
      user_comms.warn_now(message)
      user_comms.warn_later(message)
    end

    private def sqlite_error_message
      <<~ERROR
        Detected sqlite3 gem which is not supported on Heroku:
        https://devcenter.heroku.com/articles/sqlite3

      ERROR
    end

    private def gemfile_ruby_version_error
      <<~ERROR
        Detected a mismatch between your Ruby version installed and
        Ruby version specified in Gemfile or Gemfile.lock. You can
        correct this by running:

            $ bundle update --ruby
            $ git add Gemfile.lock
            $ git commit -m "update ruby version"

        If this does not solve the issue please see this documentation:

        https://devcenter.heroku.com/articles/ruby-versions#your-ruby-version-is-x-but-your-gemfile-specified-y

      ERROR
    end
  end
end
