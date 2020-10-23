require_relative "bash.rb"
require_relative "env_proxy.rb"
require_relative "curl_fetch.rb"
require_relative "user_comms.rb"

require_relative "ruby_download.rb"
require_relative "ruby_detect_version.rb"

require_relative "bundler_download.rb"
require_relative "bundler_detect_version.rb"
require_relative "bundler_strip_bundled_with.rb"


module HerokuBuildpackRuby
  # This class will detect and install a bundler version and a ruby version into a target directory
  #
  # Example:
  #
  #   vendor_dir = Pathname("/app/.heroku/ruby")
  #   prepare = PrepareAppBundlerAndRuby(
  #     vendor_dir: "/app/.heroku/ruby",
  #     app_dir: "/app",
  #     buildpack_ruby_path: `which ruby`.strip
  #   )
  #   prepare.call
  #
  #   puts vendor_dir.join("bundler/bin/bundle").exist? #=> true
  #   puts vendor_dir.join("ruby/dir/bin/ruby").exist? #=> true
  #
  #   puts ENV["PATH"].split(":").include?(vendor_dir.join("ruby/dir/bin").to_s)
  #   # => true
  #
  #   puts ENV["PATH"].split(":").include?(vendor_dir.join("bundler/bin").to_s)
  #   # => true
  class PrepareAppBundlerAndRuby
    private; attr_reader :stack, :metadata, :user_comms; public
    private; attr_reader :vendor_dir, :app_dir, :ruby_install_dir, :bundler_install_dir; public
    private; attr_reader :bundler_detect_version_object, :bundler_strip_bundled_with_object, :ruby_detect_version_object; public

    def initialize(vendor_dir: , app_dir: , buildpack_ruby_path: , user_comms: UserComms::Null.new, metadata: Metadata::Null.new, stack: ENV['STACK'])
      @stack = stack
      @app_dir = Pathname(app_dir)
      @metadata = metadata
      vendor_dir = Pathname(vendor_dir)
      @user_comms = user_comms
      lockfile_path = @app_dir.join("Gemfile.lock")
      @ruby_install_dir = vendor_dir.join("ruby")
      @bundler_install_dir = vendor_dir.join("bundler")

      @bundler_detect_version_object = BundlerDetectVersion.new(
        lockfile_path: lockfile_path
      )

      @bundler_strip_bundled_with_object = BundlerStripBundledWith.new(
        user_comms: user_comms,
        lockfile_path: lockfile_path
      )

      @ruby_detect_version_object = RubyDetectVersion.new(
        metadata: metadata,
        user_comms: user_comms,
        gemfile_dir: @app_dir,
        bundler_path: @bundler_install_dir.join("bin/bundle"),
        buildpack_ruby_path: Pathname(buildpack_ruby_path),
      )
    end

    def call
      bundler_detect_version
      bundler_download_version
      bundler_strip_bunded_with

      ruby_detect_version
      ruby_download_version

      set_env_vars

      self
    end

    def bundler_strip_bunded_with
      bundler_strip_bundled_with_object.call
    end

    def bundler_detect_version
      bundler_detect_version_object.call.version
    end

    def bundler_download_version
      BundlerDownload.new(
        version: bundler_detect_version_object.version,
        metadata: metadata,
        user_comms: user_comms,
        install_dir: bundler_install_dir
      ).call
    end

    def ruby_detect_version
      ruby_detect_version_object.call.version
    end

    def ruby_download_version
      RubyDownload.new(
        stack: stack,
        version: ruby_detect_version_object.version,
        user_comms: user_comms,
        install_dir: ruby_install_dir
      ).call
    end

    def set_env_vars
      PATH_ENV.prepend(
        ruby: ruby_install_dir.join("bin"),
        bundler: bundler_install_dir.join("bin")
      )
      GEM_PATH_ENV.prepend(
        bundler: bundler_install_dir
      )
      BUNDLE_GEMFILE_ENV.set(
        gems: app_dir.join("Gemfile").to_s
      )
    end
  end
end

