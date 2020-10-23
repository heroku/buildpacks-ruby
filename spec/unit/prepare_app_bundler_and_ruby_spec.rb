require_relative "../spec_helper.rb"

RSpec.describe "PrepareAppBundlerAndRuby" do
  describe "interfaces get called" do
    it "blerg" do
      Dir.mktmpdir do |app_dir|
        Dir.mktmpdir do |vendor_dir|
          isolate_in_fork do
            app_dir = Pathname(app_dir)
            vendor_dir = Pathname(vendor_dir)

            gemfile = Pathname(app_dir).join("Gemfile")

            gemfile.write("")

            lockfile = Pathname(app_dir).join("Gemfile.lock")
            lockfile.write <<~EOM
              RUBY VERSION
                 ruby 2.7.2p146
            EOM

            user_comms = HerokuBuildpackRuby::UserComms::Null.new
            bootstrap = HerokuBuildpackRuby::PrepareAppBundlerAndRuby.new(
              buildpack_ruby_path: which_ruby,
              vendor_dir: vendor_dir,
              user_comms: user_comms,
              app_dir: app_dir,
              stack: "heroku-18"
            )

            bootstrap.call

            output = user_comms.close.to_string
            expect(output).to include("Installing bundler")
            expect(output).to include("Removing BUNDLED WITH from Gemfile.lock")
            expect(output).to include("Using Ruby version: 2.7.2")

            expect(ENV["PATH"]).to include(vendor_dir.join("ruby").to_s)
            expect(ENV["PATH"]).to include(vendor_dir.join("bundler/bin").to_s)
            expect(ENV["GEM_PATH"]).to include(vendor_dir.join("bundler").to_s)
          end
        end
      end
    end
  end

  describe "ruby" do
    it "detects version" do
      Dir.mktmpdir do |dir|
        lockfile = Pathname(dir).join("Gemfile.lock")
        lockfile.write <<~EOM
          RUBY VERSION
             ruby 2.6.23p146
        EOM

        bootstrap = HerokuBuildpackRuby::PrepareAppBundlerAndRuby.new(
          buildpack_ruby_path: which_ruby,
          vendor_dir: "./heroku/ruby/",
          app_dir: dir,
        )
        ruby_version = bootstrap.ruby_detect_version

        expect(ruby_version).to eq("2.6.23")
      end
    end
  end

  describe "bundler" do
    it "detects major bundler version" do
      Dir.mktmpdir do |dir|
        lockfile = Pathname(dir).join("Gemfile.lock")
        lockfile.write <<~EOM
          BUNDLED WITH
             2.1.4
        EOM

        bootstrap = HerokuBuildpackRuby::PrepareAppBundlerAndRuby.new(
          buildpack_ruby_path: which_ruby,
          vendor_dir: "./heroku/ruby/",
          app_dir: dir,
        )
        bundler_version = bootstrap.bundler_detect_version

        expect(bundler_version).to eq(HerokuBuildpackRuby::BundlerDetectVersion::BUNDLER_VERSIONS["2"])
      end
    end

    it "detects version default when not specified" do
      Dir.mktmpdir do |dir|
        FileUtils.touch("#{dir}/Gemfile.lock")

        bootstrap = HerokuBuildpackRuby::PrepareAppBundlerAndRuby.new(
          buildpack_ruby_path: which_ruby,
          vendor_dir: "./heroku/ruby/bundler",
          app_dir: dir,
        )
        bundler_version = bootstrap.bundler_detect_version

        expect(bundler_version).to eq(HerokuBuildpackRuby::BundlerDetectVersion::BUNDLER_VERSIONS["1"])
      end
    end

    it "downloads bundler" do
      Dir.mktmpdir do |bundler_dest_dir|
        Dir.mktmpdir do |app_dir|
          FileUtils.touch("#{app_dir}/Gemfile.lock")

          bootstrap = HerokuBuildpackRuby::PrepareAppBundlerAndRuby.new(
            buildpack_ruby_path: which_ruby,
            vendor_dir: bundler_dest_dir,
            app_dir: app_dir,
          )

          bootstrap.bundler_detect_version
          bootstrap.bundler_download_version
          entries = Dir.entries("#{bundler_dest_dir}/bundler") - [".", ".."]
          expect(entries).to include("bin")
          expect(entries).to include("gems")
        end
      end
    end
  end
end
