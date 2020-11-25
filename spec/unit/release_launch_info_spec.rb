# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "env proxy" do
    describe "v2" do
      it "generates yaml" do
        My::Pathname.mktmpdir do |dir|
          lockfile = Object.new
          def lockfile.version(name);
            return Gem::Version.new("6.0") if name == "railties"
          end

          expect(
            ReleaseLaunchInfo::V2.new(
              lockfile: lockfile,
              vendor_dir: dir
            ).to_yaml
          ).to eq(<<~EOM)
            ---
            :default_process_types:
              :console: bin/rails console
              :web: bin/rails server -p ${PORT:-5000} -e $RAILS_ENV
          EOM
        end
      end

      it "writes yaml" do
        My::Pathname.mktmpdir do |dir|
          lockfile = Object.new
          def lockfile.version(name);
            return Gem::Version.new("6.0") if name == "railties"
          end

          ReleaseLaunchInfo::V2.new(
            lockfile: lockfile,
            vendor_dir: dir
          ).call
          expect(
            dir.join("ruby", "release.yml").read
          ).to eq(<<~EOM)
            ---
            :default_process_types:
              :console: bin/rails console
              :web: bin/rails server -p ${PORT:-5000} -e $RAILS_ENV
          EOM
        end
      end
    end

    describe "cnb" do
      it "generates toml" do
        My::Pathname.mktmpdir do |dir|
          lockfile = Object.new
          def lockfile.version(name);
            return Gem::Version.new("6.0") if name == "railties"
          end

          expect(
            ReleaseLaunchInfo::CNB.new(
              lockfile: lockfile,
              layers_dir: dir
            ).to_toml
          ).to eq(<<~EOM)
            [[processes]]
            command = "bin/rails console"
            type = :console
            [[processes]]
            command = "bin/rails server -p ${PORT:-5000} -e $RAILS_ENV"
            type = :web
          EOM
        end
      end

      it "writes toml" do
        My::Pathname.mktmpdir do |dir|
          lockfile = Object.new
          def lockfile.version(name);
            return Gem::Version.new("6.0") if name == "railties"
          end

          ReleaseLaunchInfo::CNB.new(
            lockfile: lockfile,
            layers_dir: dir
          ).call

          expect(
            dir.join("launch.toml").read
          ).to eq(<<~EOM)
            [[processes]]
            command = "bin/rails console"
            type = :console
            [[processes]]
            command = "bin/rails server -p ${PORT:-5000} -e $RAILS_ENV"
            type = :web
          EOM
        end
      end
    end
  end
end
