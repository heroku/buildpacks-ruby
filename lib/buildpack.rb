# frozen_string_literal: true

require_relative "system_bootstrap"

module Buildpack
  def self.compile(build_dir: , cache_dir:, env_dir: , buildpack_ruby_path:)
    app_dir = Pathname(build_dir)
    vendor_dir = app_dir.join(".heroku/ruby")
    profile_d_dir = app_dir.join(".profile.d/ruby.sh")

    PrepareAppBundlerAndRuby.new(
      app_dir: app_dir,
      vendor_dir: vendor_dir,
      buildpack_ruby_path: buildpack_ruby_path
    ).call

    EnvProxy.export(profile_d_dir)
  end
end
