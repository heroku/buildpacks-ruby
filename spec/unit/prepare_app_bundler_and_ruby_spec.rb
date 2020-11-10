# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "PrepareAppBundlerAndRuby" do
    describe "interfaces get called" do
      it "integration" do
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

              user_comms = UserComms::Null.new
              bootstrap = PrepareAppBundlerAndRuby.new(
                buildpack_ruby_path: which_ruby,
                bundler_install_dir: vendor_dir.join("bundler"),
                ruby_install_dir: vendor_dir.join("ruby"),
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

          vendor_dir = Pathname(dir).join(".heroku/")
          bootstrap = PrepareAppBundlerAndRuby.new(
            buildpack_ruby_path: which_ruby,
            bundler_install_dir: vendor_dir.join("bundler"),
            ruby_install_dir: vendor_dir.join("ruby"),
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

          vendor_dir = Pathname(dir).join(".heroku/")
          bootstrap = PrepareAppBundlerAndRuby.new(
            buildpack_ruby_path: which_ruby,
            bundler_install_dir: vendor_dir.join("bundler"),
            ruby_install_dir: vendor_dir.join("ruby"),
            app_dir: dir,
          )
          bundler_version = bootstrap.bundler_detect_version

          expect(bundler_version).to eq(BundlerDetectVersion::BUNDLER_VERSIONS["2"])
        end
      end

      it "downloads bundler" do
        Dir.mktmpdir do |bundler_dest_dir|
          bundler_dest_dir = Pathname(bundler_dest_dir)
          Dir.mktmpdir do |app_dir|
            app_dir = Pathname(app_dir)
            app_dir.join("Gemfile.lock").tap {|p| FileUtils.touch(p) }

            vendor_dir = app_dir.join(".heroku/")
            bootstrap = PrepareAppBundlerAndRuby.new(
              buildpack_ruby_path: which_ruby,
              bundler_install_dir: bundler_dest_dir,
              ruby_install_dir: vendor_dir.join("ruby"),
              app_dir: app_dir,
            )

            bootstrap.bundler_detect_version
            bootstrap.bundler_download_version

            expect(bundler_dest_dir.entries.map(&:to_s)).to include("bin")
            expect(bundler_dest_dir.entries.map(&:to_s)).to include("gems")
          end
        end
      end
    end
  end
end
