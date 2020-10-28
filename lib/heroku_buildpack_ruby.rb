require_relative "heroku_buildpack_ruby/prepare_app_bundler_and_ruby.rb"
require_relative "heroku_buildpack_ruby/bundler_lockfile_parser.rb"
require_relative "heroku_buildpack_ruby/bundle_install.rb"
require_relative "heroku_buildpack_ruby/cache_copy.rb"
require_relative "heroku_buildpack_ruby/metadata.rb"

require_relative "heroku_buildpack_ruby/rake_detect.rb"

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
  class BuildpackErrorNoBacktrace < StandardError; end

  BUILDPACK_DIR = Pathname(__dir__).join("..")
  EnvProxy.register_layer(:gems,    build: true, cache: true,  launch: true)
  EnvProxy.register_layer(:bundler, build: true, cache: false, launch: true)
  EnvProxy.register_layer(:ruby,    build: true, cache: false, launch: true)

  def self.compile_legacy(build_dir: , cache_dir:, env_dir: , buildpack_ruby_path:)
    export = BUILDPACK_DIR.join("export")
    app_dir = Pathname(build_dir)
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


    gems_cache_copy.call do |gems_dir|
      BundleInstall.new(
        app_dir: app_dir,
        metadata: metadata,
        user_comms: user_comms,
        bundle_without_default: "development:test",
        bundle_install_gems_dir: gems_dir,
      ).call
    end

    EnvProxy.export(
      app_dir: app_dir,
      export_path: export,
      profile_d_path: profile_d_path,
    )
    user_comms.close
  rescue BuildpackErrorNoBacktrace => e
    user_comms.error_and_exit(e.message)
  end

  def self.build_cnb(layers_dir: , platform_dir: , env_dir: , plan: , app_dir: , buildpack_ruby_path:)
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

    BundleInstall.new(
      app_dir: app_dir,
      metadata: metadata,
      user_comms: user_comms,
      bundle_without_default: "development:test",
      bundle_install_gems_dir: gems_install_dir,
    ).call

    EnvProxy.write_layers(
      layers_dir: layers_dir
    )
    user_comms.close

  rescue BuildpackErrorNoBacktrace => e
    user_comms.error_and_exit(e.message)
  end

  class AssetsPrecompile
    def initialize(rake: , app_dir: , user_comms: UserComms::Null.new)
      @rake = rake
      @user_comms = user_comms
      @public_dir = Pathname(app_dir).join("public/assets")
    end

    def call
      case
      when assets_manifest
        @user_comms.puts("Skipping `rake assets:precompile`: Asset manifest found: #{assets_manifest}")
      else
        assets_precompile
        assets_clean
      end
    end

    private def assets_precompile
      if rake.detect?("assets:precompile")
        @user_comms.topic("Running: rake assets:precompile")
        RakeTask.new("assets:precompile", stream: @user_comms).call
      else
        @user_comms.puts("Asset compilation skipped: `rake assets:precompile` not found")
      end
    end

    private def assets_clean
      if rake.detect?("assets:clean")
        @user_comms.topic("Running: rake assets:clean")

        RakeTask.new("assets:precompile", stream: @user_comms).call
      else
        @user_comms.puts("Asset clean skipped: `rake assets:clean` not found")
      end
    end

    private def asset_manifest
      @public_dir.glob(manifest_glob_pattern).first
    end

    private def manifest_glob_pattern
      files_string = [".sprockets-manifest-*.json", "manifest-*.json", "manifest.yml"].join(",")
      "{#{files_string}}"
    end
  end
end
