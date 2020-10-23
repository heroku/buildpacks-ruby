require_relative "heroku_buildpack_ruby/prepare_app_bundler_and_ruby.rb"
require_relative "heroku_buildpack_ruby/bundler_lockfile_parser.rb"
require_relative "heroku_buildpack_ruby/bundle_install.rb"
require_relative "heroku_buildpack_ruby/cache_copy.rb"
require_relative "heroku_buildpack_ruby/metadata.rb"

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

    metadata = Metadata.new(dir: metadata_dir, type: Metadata::V2)
    user_comms = UserComms::V2.new
    gems_cache_copy = CacheCopy.new(cache_dir: gems_cache_dir, dest_dir: gems_install_dir)

    PrepareAppBundlerAndRuby.new(
      app_dir: app_dir,
      metadata: metadata,
      vendor_dir: vendor_dir,
      user_comms: user_comms,
      buildpack_ruby_path: buildpack_ruby_path,
    ).call

    # TODO detect and install binary dependencies here

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
  end

  def self.build_cnb(layers_dir: , platform_dir: , env_dir: , plan: , app_dir: , buildpack_ruby_path:)
    app_dir = Pathname(app_dir)
    layers_dir = Pathname(layers_dir)
    vendor_dir = app_dir.join(".heroku/ruby")
    gems_install_dir = layers_dir.join("gems")

    metadata = Metadata.new(dir: layers_dir, type: Metadata::CNB)
    user_comms = UserComms::CNB.new

    PrepareAppBundlerAndRuby.new(
      app_dir: app_dir,
      metadata: metadata,
      vendor_dir: vendor_dir, # TODO move to layers
      user_comms: user_comms,
      buildpack_ruby_path: buildpack_ruby_path,
    ).call

    # TODO detect and install binary dependencies here

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
  end
end
