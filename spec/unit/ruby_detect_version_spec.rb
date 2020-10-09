require_relative "../spec_helper.rb"

RSpec.describe "detect ruby version" do
  it "matches on lockfile" do
    Dir.mktmpdir do |dir|
      lockfile = Pathname.new(dir).join("Gemfile.lock")
      lockfile.write <<~EOM
        PLATFORMS
          ruby

        DEPENDENCIES
          heroku_hatchet
          parallel_split_test
          rspec-retry

        RUBY VERSION
           ruby 2.7.2p137

        BUNDLED WITH
           2.1.4
      EOM
      ruby_version = HerokuBuildpackRuby::RubyDetectVersion.new(
        buildpack_ruby_path: which_ruby,
        bundler_path: which_bundle,
        gemfile_dir: dir
      )
      ruby_version.call
      expect(ruby_version.version).to eq("2.7.2")
    end
  end

  it "detects from Gemfile" do
    Dir.mktmpdir do |dir|

      File.open("#{dir}/Gemfile", "w+") do |f|
        f.write "ruby '2.7.6'"
      end
      FileUtils.touch("#{dir}/Gemfile.lock")

      ruby_version = HerokuBuildpackRuby::RubyDetectVersion.new(
        buildpack_ruby_path: which_ruby,
        bundler_path: which_bundle,
        gemfile_dir: dir
      )

      # We need a clean environment, we don't want to run bundler inside of another bundler
      Bundler.with_unbundled_env do
        ruby_version.call
        expect(ruby_version.version).to eq("2.7.6")
      end
    end
  end

  it "defaults if empty" do
    Dir.mktmpdir do |dir|

      FileUtils.touch("#{dir}/Gemfile.lock")
      FileUtils.touch("#{dir}/Gemfile")

      ruby_version = HerokuBuildpackRuby::RubyDetectVersion.new(
        buildpack_ruby_path: which_ruby,
        bundler_path: which_bundle,
        gemfile_dir: dir
      )

      # We need a clean environment, we don't want to run bundler inside of another bundler
      Bundler.with_unbundled_env do
        ruby_version.call
        expect(ruby_version.version).to eq(HerokuBuildpackRuby::RubyDetectVersion::DEFAULT)
      end
    end
  end
end
