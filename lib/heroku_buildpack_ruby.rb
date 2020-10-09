require_relative "heroku_buildpack_ruby/prepare_app_bundler_and_ruby.rb"

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
    user_output = UserOutput::V2.new
    profile_d_path = app_dir.join(".profile.d/ruby.sh")

    PrepareAppBundlerAndRuby.new(
      app_dir: app_dir,
      vendor_dir: vendor_dir,
      user_output: user_output,
      buildpack_ruby_path: buildpack_ruby_path,
    ).call

    EnvProxy.export(
      export: export,
      app_dir: app_dir,
      profile_d: profile_d_path,
    )
  end

  def self.build_cnb(layers_dir: , platform_dir: , env_dir: , plan: , app_dir: , buildpack_ruby_path:)
    app_dir = Pathname.new(app_dir)
    layers_dir = Pathname.new(layers_dir)
    vendor_dir = app_dir.join(".heroku/ruby")
    user_output = UserOutput::CNB.new

    PrepareAppBundlerAndRuby.new(
      app_dir: app_dir,
      vendor_dir: vendor_dir,
      user_output: user_output,
      buildpack_ruby_path: buildpack_ruby_path,
    ).call

    EnvProxy.write_layers(
      layers_dir: layers_dir
    )
  end
end
