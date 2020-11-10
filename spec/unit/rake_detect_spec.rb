# frozen_string_literal: true

require_relative "../spec_helper.rb"


module HerokuBuildpackRuby
  RSpec.describe "rake detect" do
    describe "failures and warnings" do
      it "detects when there is no rake gem" do
        Hatchet::Runner.new("default_ruby").in_directory do
          app_dir = Pathname(Dir.pwd)
          app_dir.join("Rakefile").delete

          Bundler.with_original_env do
            user_comms = UserComms::Null.new
            rake = RakeDetect.new(
              app_dir: app_dir,
              user_comms: user_comms,
              has_rake_gem: false
            ).call

            expect(user_comms.to_string).to include("No `rake` gem")
            expect(rake.loaded?).to be_falsey
            expect(rake.detect?("assets:precompile")).to be_falsey

            expect {
              rake = RakeDetect.new(
                app_dir: app_dir,
                has_rake_gem: false,
                error_if_detect_fails: true
              ).call
            }.to raise_error(/No `rake` gem/)
          end
        end
      end

      it "detects when there is no Rakefile" do

        Hatchet::Runner.new("default_ruby").in_directory do
          app_dir = Pathname(Dir.pwd)
          app_dir.join("Rakefile").delete

          Bundler.with_original_env do
            user_comms = UserComms::Null.new
            rake = RakeDetect.new(
              app_dir: app_dir,
              user_comms: user_comms,
              has_rake_gem: true
            ).call

            expect(user_comms.to_string).to include("No Rakefile found")
            expect(rake.loaded?).to be_falsey
            expect(rake.detect?("assets:precompile")).to be_falsey

            expect {
              rake = RakeDetect.new(
                app_dir: app_dir,
                has_rake_gem: true,
                error_if_detect_fails: true
              ).call
            }.to raise_error(/No Rakefile found/)
          end
        end
      end

      it "detects when there is an exception in the Rakefile" do
        Hatchet::Runner.new("default_ruby").in_directory do
          app_dir = Pathname(Dir.pwd)
          app_dir.join("Rakefile").write <<~EOM
            raise "nope"
          EOM

          Bundler.with_original_env do
            user_comms = UserComms::Null.new
            rake = RakeDetect.new(
              app_dir: app_dir,
              user_comms: user_comms,
              has_rake_gem: true
            ).call

            expect(user_comms.to_string).to include("Rake task detection failed")
            expect(rake.loaded?).to be_falsey
            expect(rake.detect?("assets:precompile")).to be_falsey

            expect {
              rake = RakeDetect.new(
                app_dir: app_dir,
                has_rake_gem: true,
                error_if_detect_fails: true
              ).call
            }.to raise_error(/Rake task detection failed/)
          end
        end
      end
    end


    it "detects when assets:precompile is not present" do
      Hatchet::Runner.new("default_ruby").in_directory do
        app_dir = Pathname(Dir.pwd)
        app_dir.join("Rakefile").write <<~EOM
         # Empty
        EOM

        Bundler.with_original_env do
          rake = RakeDetect.new(
            app_dir: app_dir,
            has_rake_gem: true
          ).call

          expect(rake.loaded?).to be_truthy
          expect(rake.detect?("assets:precompile")).to be_falsey
        end
      end
    end

    it "detects asset tasks when they exist" do
      Hatchet::Runner.new("default_ruby").in_directory do
        app_dir = Pathname(Dir.pwd)
        app_dir.join("Rakefile").write <<~EOM
          task "assets:precompile" do
            puts "success!"
          end
        EOM

        Bundler.with_original_env do
          rake = RakeDetect.new(
            app_dir: app_dir,
            has_rake_gem: true
          ).call

          expect(rake.loaded?).to be_truthy
          expect(rake.detect?("assets:precompile")).to be_truthy
        end
      end
    end
  end
end
