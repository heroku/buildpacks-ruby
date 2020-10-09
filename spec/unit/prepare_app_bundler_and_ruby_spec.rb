require_relative "../spec_helper.rb"

RSpec.describe "PrepareAppBundlerAndRuby" do
  it "detects major bundler version" do
    Dir.mktmpdir do |dir|
      lockfile = Pathname.new(dir).join("Gemfile.lock")
      lockfile.write(<<~EOM
        BUNDLED WITH
           2.1.4
      EOM
      )
      user_output = HerokuBuildpackRuby::UserOutput::V2.new(StringIO.new)
      bootstrap = HerokuBuildpackRuby::PrepareAppBundlerAndRuby.new(
        buildpack_ruby_path: which_ruby,
        vendor_dir: "./heroku/ruby/",
        app_dir: dir,
        user_output: user_output
      )
      bundler_version = bootstrap.detect_bundler_version!

      expect(bundler_version).to eq(HerokuBuildpackRuby::BundlerDetectVersion::BUNDLER_VERSIONS["2"])
    end
  end

  it "detects bundler version default when not specified" do
    Dir.mktmpdir do |dir|
      FileUtils.touch("#{dir}/Gemfile.lock")

      user_output = HerokuBuildpackRuby::UserOutput::V2.new(StringIO.new)
      bootstrap = HerokuBuildpackRuby::PrepareAppBundlerAndRuby.new(
        buildpack_ruby_path: which_ruby,
        vendor_dir: "./heroku/ruby/bundler",
        app_dir: dir,
        user_output: user_output
      )
      bundler_version = bootstrap.detect_bundler_version!

      expect(bundler_version).to eq(HerokuBuildpackRuby::BundlerDetectVersion::BUNDLER_VERSIONS["1"])
    end
  end

  it "downloads bundler" do
    Dir.mktmpdir do |bundler_dest_dir|
      Dir.mktmpdir do |app_dir|
        FileUtils.touch("#{app_dir}/Gemfile.lock")

      user_output = HerokuBuildpackRuby::UserOutput::V2.new(StringIO.new)
        bootstrap = HerokuBuildpackRuby::PrepareAppBundlerAndRuby.new(
          buildpack_ruby_path: which_ruby,
          vendor_dir: bundler_dest_dir,
          app_dir: app_dir,
          user_output: user_output
        )

        bootstrap.detect_bundler_version!
        bootstrap.download_bundler_version!
        entries = Dir.entries("#{bundler_dest_dir}/bundler") - [".", ".."]
        expect(entries).to include("bin")
        expect(entries).to include("gems")
      end
    end
  end
end
