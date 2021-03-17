# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "assets_precompile" do
    it "streams the contents of the rake task to the output" do
      My::Pathname.mktmpdir do |dir|
        Dir.chdir(dir) do
          dir.join("Rakefile").write <<~EOM
            task "assets:precompile" do
              puts "woop woop precompile worked"
            end

            task "assets:clean" do
              puts "woop woop clean worked"
            end
          EOM

          user_comms = UserComms::Null.new
          Bundler.with_original_env do
            AssetsPrecompile.new(
              has_assets_precompile: true,
              has_assets_clean: true,
              user_comms: user_comms,
              app_dir: dir,
            ).call
          end

          expect(user_comms.to_string).to include("rake assets:precompile")
          expect(user_comms.to_string).to include("woop woop precompile worked")
          expect(user_comms.to_string).to include("woop woop clean worked")
        end
      end
    end

    it "fails gracefully" do
      My::Pathname.mktmpdir do |dir|
        Dir.chdir(dir) do
          dir.join("Rakefile").write <<~EOM
            task "assets:precompile" do
              raise "Nope"
            end

            task "assets:clean" do
              puts "woop woop clean worked"
            end
          EOM

          user_comms = UserComms::Null.new
          assets_precompile = AssetsPrecompile.new(
            has_assets_precompile: true,
            has_assets_clean: true,
            user_comms: user_comms,
            app_dir: dir,
          )

          expect {
            Bundler.with_original_env do
              assets_precompile.call
            end
          }.to raise_error {|error|
            expect(error).to be_a(BuildpackErrorNoBacktrace)
            expect(error.title).to eq("Precompiling assets failed")
            expect(error.body).to include("Nope")
          }

          # Gives specific help when database is not provisioned
          expect {
            assets_precompile.precompile_fail(output: "127.0.0.1")
          }.to raise_error {|error|
            expect(error).to be_a(BuildpackErrorNoBacktrace)
            expect(error.link).to eq("https://devcenter.heroku.com/articles/pre-provision-database")
          }
        end
      end
    end

    it "skips asset compilation when manifest is found" do
      My::Pathname.mktmpdir do |dir|
        public_dir = dir.join("public/assets").tap(&:mkpath)

        FileUtils.touch(public_dir.join(".sprockets-manifest-asdf.json"))

        user_comms = UserComms::Null.new
        AssetsPrecompile.new(
          has_assets_precompile: false,
          has_assets_clean: true,
          user_comms: user_comms,
          app_dir: dir,
        ).call

        expect(user_comms.to_string).to include("asset manifest found")
      end
    end

    it "skips asset compilation when task is not found" do
      My::Pathname.mktmpdir do |dir|
        rake = Object.new
        def rake.detect?(value); return false if value == "assets:precompile"; end

        user_comms = UserComms::Null.new
        AssetsPrecompile.new(
          has_assets_precompile: false,
          has_assets_clean: true,
          user_comms: user_comms,
          app_dir: dir,
        ).call

        expect(user_comms.to_string).to include("Asset compilation skipped")
      end
    end
  end
end
