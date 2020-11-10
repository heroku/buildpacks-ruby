# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "BundleInstall" do
    it "sets env vars and does not clear cache if nothing changed" do
      Hatchet::Runner.new("default_ruby").in_directory_fork do
        stringio = StringIO.new
        app_dir = Pathname(Dir.pwd)
        gems_dir = app_dir.join(".heroku/ruby/gems").tap(&:mkpath)

        # The Bundler.with_original_env will Reset bundler's env vars since we're running in a `bundle exec` context for tests already
        Bundler.with_original_env do
          BundleInstall.new(
            app_dir: Dir.pwd,
            bundle_without_default: "development:test",
            bundle_install_gems_dir: gems_dir,
            user_comms: UserComms::V2.new(stringio)
          ).call

          expect(stringio.string).to include("bundle install")
          expect(stringio.string).to include("Fetching")
          expect(stringio.string).to include("Cleaning up the bundler cache")
          expect(stringio.string).to include("Fetching rack")

          expect(ENV.key?("BUNDLE_DEPLOYMENT")).to be_truthy

          expect(ENV["GEM_PATH"]).to include(gems_dir.to_s)
          expect(ENV["BUNDLE_BIN"]).to include(gems_dir.join("bin").to_s)
          expect(ENV["BUNDLE_PATH"]).to include(gems_dir.to_s)
          expect(ENV["PATH"]).to include(gems_dir.join("bin").to_s)

          expect(ENV["BUNDLE_WITHOUT"]).to eq("development:test")
          expect(ENV["BUNDLE_GEMFILE"]).to eq(app_dir.join("Gemfile").to_s)

          # Test that the cache is not called unless gems were fetched
          stringio.reopen
          BundleInstall.new(
            app_dir: Dir.pwd,
            bundle_without_default: "development:test",
            bundle_install_gems_dir: gems_dir,
            user_comms: UserComms::V2.new(stringio)
          ).call

          expect(stringio.string).to include("bundle install")
          expect(stringio.string).to include("Skipping cleaning")

          expect(stringio.string).to_not include("Fetching")
          expect(stringio.string).to_not include("Cleaning up the bundler cache")
        end
      end
    end

    it "handles BUNDLE_WITHOUT with a space" do
      Hatchet::Runner.new("default_ruby").in_directory_fork do
        stringio = StringIO.new
        app_dir = Pathname(Dir.pwd)
        gems_dir = app_dir.join(".heroku/ruby/gems").tap(&:mkpath)

        Bundler.with_original_env do
          ENV["BUNDLE_WITHOUT"] = "i have spaces"
          BundleInstall.new(
            app_dir: Dir.pwd,
            bundle_without_default: "development:test",
            bundle_install_gems_dir: gems_dir,
            user_comms: UserComms::V2.new(stringio)
          ).call

          expect(stringio.string).to include("Warning")
          expect(stringio.string).to include('BUNDLE_WITHOUT="i:have:spaces"')
        end
      end
    end

    it "handles user defined BUNDLE_WITHOUT" do
      Hatchet::Runner.new("default_ruby").in_directory_fork do
        stringio = StringIO.new
        app_dir = Pathname(Dir.pwd)
        gems_dir = app_dir.join(".heroku/ruby/gems").tap(&:mkpath)

        Dir.mktmpdir do |dir|
          dir = Pathname(dir)
          dir.join("BUNDLE_WITHOUT").write("i have spaces")
          UserEnv.parse(dir)
        end

        Bundler.with_original_env do
          BundleInstall.new(
            app_dir: Dir.pwd,
            bundle_without_default: "development:test",
            bundle_install_gems_dir: gems_dir,
            user_comms: UserComms::V2.new(stringio)
          ).call

          expect(stringio.string).to include("Warning")
          expect(stringio.string).to include('BUNDLE_WITHOUT="i:have:spaces"')
          expect(stringio.string).to include('heroku config:set BUNDLE_WITHOUT="i:have:spaces"')
        ensure
          UserEnv.clear
        end
      end
    end
  end
end
