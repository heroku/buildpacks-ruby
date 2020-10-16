require_relative "heroku_buildpack_ruby/prepare_app_bundler_and_ruby.rb"
require_relative "heroku_buildpack_ruby/bundle_install.rb"

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
  BUILDPACK_DIR = Pathname.new(__dir__).join("..")
  EnvProxy.register_layer(:gems,    build: true, cache: true,  launch: true)
  EnvProxy.register_layer(:bundler, build: true, cache: false, launch: true)
  EnvProxy.register_layer(:ruby,    build: true, cache: false, launch: true)

  def self.compile_legacy(build_dir: , cache_dir:, env_dir: , buildpack_ruby_path:)
    export = BUILDPACK_DIR.join("export")
    app_dir = Pathname.new(build_dir)
    vendor_dir = app_dir.join(".heroku/ruby")
    user_comms = UserComms::V2.new
    profile_d_path = app_dir.join(".profile.d/ruby.sh")

    PrepareAppBundlerAndRuby.new(
      app_dir: app_dir,
      vendor_dir: vendor_dir,
      user_comms: user_comms,
      buildpack_ruby_path: buildpack_ruby_path,
    ).call

    BundleInstall.new(
      app_dir: app_dir,
      user_comms: user_comms,
      bundle_without_default: "development:test",
      bundle_install_gems_dir: vendor_dir.join("gems"),
    ).call

    EnvProxy.export(
      app_dir: app_dir,
      export_path: export,
      profile_d_path: profile_d_path,
    )
  end

  def self.build_cnb(layers_dir: , platform_dir: , env_dir: , plan: , app_dir: , buildpack_ruby_path:)
    app_dir = Pathname.new(app_dir)
    layers_dir = Pathname.new(layers_dir)
    vendor_dir = app_dir.join(".heroku/ruby")
    user_comms = UserComms::CNB.new

    PrepareAppBundlerAndRuby.new(
      app_dir: app_dir,
      vendor_dir: vendor_dir,
      user_comms: user_comms,
      buildpack_ruby_path: buildpack_ruby_path,
    ).call

    BundleInstall.new(
      app_dir: app_dir,
      user_comms: user_comms,
      bundle_without_default: "development:test",
      bundle_install_gems_dir: vendor_dir.join("gems"),
    ).call

    EnvProxy.write_layers(
      layers_dir: layers_dir
    )
  end
end
