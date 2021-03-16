# frozen_string_literal: true

require_relative "heroku_buildpack_ruby/user_env_from_dir.rb"

require_relative "heroku_buildpack_ruby/prepare_app_bundler_and_ruby.rb"
require_relative "heroku_buildpack_ruby/bundler_lockfile_parser.rb"
require_relative "heroku_buildpack_ruby/bundle_install.rb"
require_relative "heroku_buildpack_ruby/cache_copy.rb"
require_relative "heroku_buildpack_ruby/metadata.rb"

require_relative "heroku_buildpack_ruby/rake_detect.rb"
require_relative "heroku_buildpack_ruby/assets_precompile.rb"
require_relative "heroku_buildpack_ruby/set_default_env_vars.rb"

require_relative "heroku_buildpack_ruby/release_launch_info.rb"

# This is the main entry point for the Ruby buildpack
#
# Legacy/V2 interface:
#
#   HerokuBuildpackRuby.compile_legacy(...)
#
# CNB interface:
#
#   HerokuBuildpackRuby.build_cnb(...)
#
module HerokuBuildpackRuby
  class BuildpackErrorNoBacktrace < StandardError
    attr_reader :title, :body, :link
    def initialize(title: , body:, link: nil)
      @title = title
      @body = body
      @link = link
      super [title, body, link].join("\n")
    end
  end

  BUILDPACK_DIR = Pathname(__dir__).join("..")
  EnvProxy.register_layer(:gems,    build: true, cache: true,  launch: true)
  EnvProxy.register_layer(:bundler, build: true, cache: false, launch: true)
  EnvProxy.register_layer(:ruby,    build: true, cache: false, launch: true)

  def self.compile_legacy(build_dir: , cache_dir:, env_dir: , buildpack_ruby_path:)
    app_dir = Pathname(build_dir)
    UserEnv.parse(env_dir)

    Dir.chdir(app_dir) do
      export = BUILDPACK_DIR.join("export")
      cache_dir = Pathname(cache_dir)
      vendor_dir = app_dir.join(".heroku/ruby")
      metadata_dir = cache_dir.join("vendor/heroku")
      profile_d_path = app_dir.join(".profile.d/ruby.sh")

      gems_cache_dir = cache_dir.join("gems")
      gems_install_dir = vendor_dir.join("gems")
      ruby_install_dir = vendor_dir.join("ruby")
      gemfile_lock_path = app_dir.join("Gemfile.lock")
      bundler_install_dir = vendor_dir.join("bundler")

      metadata = Metadata.new(dir: metadata_dir, type: Metadata::V2)
      user_comms = UserComms::V2.new
      gems_cache_copy = CacheCopy.new(cache_dir: gems_cache_dir, dest_dir: gems_install_dir)

      PrepareAppBundlerAndRuby.new(
        app_dir: app_dir,
        metadata: metadata,
        user_comms: user_comms,
        ruby_install_dir: ruby_install_dir,
        bundler_install_dir: bundler_install_dir,
        buildpack_ruby_path: buildpack_ruby_path,
      ).call

      SetDefaultEnvVars.new(
        metadata: metadata,
        environment: "production"
      ).call

      gems_cache_copy.call do |gems_dir|
        BundleInstall.new(
          app_dir: app_dir,
          metadata: metadata,
          user_comms: user_comms,
          bundle_without_default: "development:test",
          bundle_install_gems_dir: gems_dir,
        ).call
      end

      lockfile = HerokuBuildpackRuby::BundlerLockfileParser.new(
        gemfile_lock_path: gemfile_lock_path,
        bundler_install_dir: bundler_install_dir,
      ).call

      rake = HerokuBuildpackRuby::RakeDetect.new(
        app_dir: app_dir,
        user_comms: user_comms,
        has_rake_gem: lockfile.has_gem?("rake"),
        error_if_detect_fails: lockfile.has_gem?("sprockets"),
      ).call

      # TODO caching
      HerokuBuildpackRuby::AssetsPrecompile.new(
        app_dir: app_dir,
        user_comms: user_comms,
        has_assets_clean: rake.detect?("assets:clean"),
        has_assets_precompile: rake.detect?("assets:precompile"),
      ).call

      ReleaseLaunchInfo::V2.new(
        lockfile: lockfile,
        vendor_dir: vendor_dir
      ).call
      EnvProxy.export(
        app_dir: app_dir,
        export_path: export,
        profile_d_path: profile_d_path,
      )
      user_comms.close
    rescue BuildpackErrorNoBacktrace => e
      user_comms.print_error_obj(e)
      exit(1)
    end
  end

  def self.build_cnb(layers_dir: , platform_dir: , env_dir: , plan: , app_dir: , buildpack_ruby_path:)
    UserEnv.parse(env_dir)

    Dir.chdir(app_dir) do
      app_dir = Pathname(app_dir)
      layers_dir = Pathname(layers_dir)
      gems_install_dir = layers_dir.join("gems")
      ruby_install_dir = layers_dir.join("ruby")
      gemfile_lock_path = app_dir.join("Gemfile.lock")
      bundler_install_dir = layers_dir.join("bundler")

      metadata = Metadata.new(dir: layers_dir, type: Metadata::CNB)
      user_comms = UserComms::CNB.new

      PrepareAppBundlerAndRuby.new(
        app_dir: app_dir,
        metadata: metadata,
        user_comms: user_comms,
        ruby_install_dir: ruby_install_dir,
        bundler_install_dir: bundler_install_dir,
        buildpack_ruby_path: buildpack_ruby_path,
      ).call


      SetDefaultEnvVars.new(
        metadata: metadata,
        environment: "production"
      ).call

      BundleInstall.new(
        app_dir: app_dir,
        metadata: metadata,
        user_comms: user_comms,
        bundle_without_default: "development:test",
        bundle_install_gems_dir: gems_install_dir,
      ).call

      lockfile = HerokuBuildpackRuby::BundlerLockfileParser.new(
        gemfile_lock_path: gemfile_lock_path,
        bundler_install_dir: bundler_install_dir,
      ).call

      rake = HerokuBuildpackRuby::RakeDetect.new(
        app_dir: app_dir,
        user_comms: user_comms,
        has_rake_gem: lockfile.has_gem?("rake"),
        error_if_detect_fails: lockfile.has_gem?("sprockets"),
      ).call

      # TODO caching
      HerokuBuildpackRuby::AssetsPrecompile.new(
        app_dir: app_dir,
        user_comms: user_comms,
        has_assets_clean: rake.detect?("assets:clean"),
        has_assets_precompile: rake.detect?("assets:precompile"),
      ).call

      ReleaseLaunchInfo::CNB.new(
        lockfile: lockfile,
        layers_dir: layers_dir
      ).call
      EnvProxy.write_layers(
        layers_dir: layers_dir
      )
      user_comms.close
    rescue BuildpackErrorNoBacktrace => e
      user_comms.print_error_obj(e)
      exit(1)
    end
  end
end
