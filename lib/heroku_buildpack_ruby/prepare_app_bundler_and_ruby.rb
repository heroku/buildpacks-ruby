require_relative "env_proxy.rb"
require_relative "bash.rb"
require_relative "curl_fetch.rb"
require_relative "user_comms.rb"

require_relative "ruby_detect_version.rb"
require_relative "ruby_download.rb"

require_relative "bundler_detect_version.rb"
require_relative "bundler_download.rb"


module HerokuBuildpackRuby
  # This class will detect and install a bundler version and a ruby version into a target directory
  #
  # Example:
  #
  #   vendor_dir = Pathname.new("/app/.heroku/ruby")
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
    private; attr_reader :user_comms, :vendor_dir, :app_dir, :ruby_install_dir, :bundler_install_dir, :bundler_detect_version, :ruby_detect_version; public
    public; attr_reader :gem_install_dir

    def initialize(vendor_dir: , app_dir: , buildpack_ruby_path: , user_comms: UserComms::V2.new, metadata: MetadataNull.new)
      @app_dir = Pathname.new(app_dir)
      @metadata = metadata
      @vendor_dir = Pathname.new(vendor_dir)
      @user_comms = user_comms

      @gem_install_dir = @vendor_dir.join("gems")
      @ruby_install_dir = @vendor_dir.join("dir")
      @bundler_install_dir = @vendor_dir.join("bundler")

      @bundler_detect_version = BundlerDetectVersion.new(
        lockfile_path: @app_dir.join("Gemfile.lock")
      )

      @ruby_detect_version = RubyDetectVersion.new(
        metadata: metadata,
        gemfile_dir: @app_dir,
        buildpack_ruby_path: Pathname.new(buildpack_ruby_path),
        bundler_path: @bundler_install_dir.join("bin/bundle")
      )
    end

    def call
      detect_bundler_version!
      download_bundler_version!

      detect_ruby_version!
      download_ruby_version!

      configure_ruby_and_bundler_env_vars!

      self
    end

    def detect_bundler_version!
      @bundler_detect_version.call
      bundler_version = @bundler_detect_version.version
      @user_comms.topic("Installing bundler #{bundler_version}")

      # TODO remove BUNDLE WITH version in Gemfile.lock
      # @user.topic("Removing BUNDLED WITH version in the Gemfile.lock")
      bundler_version
    end

    def download_bundler_version!
      BundlerDownload.new(
        version: @bundler_detect_version.version,
        install_dir: @bundler_install_dir
      ).call
    end

    def detect_ruby_version!
      @ruby_detect_version.call
    end

    def download_ruby_version!
      RubyDownload.new(
        version: @ruby_detect_version.version,
        install_dir: @ruby_install_dir
      ).call
    end

    def configure_ruby_and_bundler_env_vars!
      PATH_ENV.prepend(
        ruby: @ruby_install_dir.join("bin"),
        bundler: @bundler_install_dir.join("bin")
      )
      GEM_PATH_ENV.prepend(
        bundler: @bundler_install_dir
      )
      BUNDLE_GEMFILE_ENV.set(
        gems: @app_dir.join("Gemfile").to_s
      )
    end
  end
end

