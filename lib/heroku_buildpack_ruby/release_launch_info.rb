# frozen_string_literal: true

require "yaml"
require_relative "default_process_types.rb"

module HerokuBuildpackRuby
  class ReleaseLaunchInfo
    # Writes out info for a release (v2) or launch (CNB)
    #
    #   ReleaseLaunchInfo::V2.new(
    #     lockfile: lockfile,
    #     vendor_dir: vendor_dir
    #   ).call
    #
    #   expect(vendor_dir.read).to eq(<<~EOM)
    #     ---
    #     :default_process_types:
    #       :console: bin/rails console
    #       :web: bin/rails server -p ${PORT:-5000} -e $RAILS_ENV
    #   EOM
    class V2
      def initialize(lockfile:, vendor_dir:)
        @release_yml_path = Pathname(vendor_dir).join("release.yml").tap {|p| p.dirname.mkpath; FileUtils.touch(p)}
        @process_types = DefaultProcessTypes.new(lockfile)
      end

      def to_yaml
        yaml = YAML.load(@release_yml_path.read) || {}
        yaml[:default_process_types] = @process_types.to_h
        YAML.dump(yaml)
      end

      def call
        @release_yml_path.write(to_yaml)
      end
    end

    # Writes out info for a launch (CNB)
    #
    #   ReleaseLaunchInfo::V2.new(
    #     lockfile: lockfile,
    #     layers_dir: layers_dir
    #   ).call
    #
    #
    #   expect(vendor_dir.read).to eq(<<~EOM)
    #     [[processes]]
    #     command = "bin/rails console"
    #     type = :console
    #     [[processes]]
    #     command = "bin/rails server -p ${PORT:-5000} -e $RAILS_ENV"
    #     type = :web
    #   EOM
    class CNB
      def initialize(lockfile:, layers_dir:)
        @launch_toml_path = Pathname(layers_dir).join("launch.toml").tap {|p| p.dirname.mkpath; FileUtils.touch(p)}
        @process_types = DefaultProcessTypes.new(lockfile)
      end

      def to_toml
        toml = TOML.load(@launch_toml_path) || {}

        toml[:processes] = @process_types.to_h.map do |type, command|
          { type: type, command: command}
        end

        TOML.dump(toml)
      end

      def call
        @launch_toml_path.write(to_toml)
      end
    end
  end
end
