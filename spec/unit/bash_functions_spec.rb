# frozen_string_literal: true

require_relative "../spec_helper.rb"


module HerokuBuildpackRuby
  RSpec.describe "bash_functions.sh" do
    def exec_with_bash_functions(code, stack: "heroku-18")
      contents = <<~EOM
        #! /usr/bin/env bash
        set -eu

        STACK="#{stack}"

        #{bash_functions_file.read}

        #{code}
      EOM

      file = Tempfile.new
      file.write(contents)
      file.close
      FileUtils.chmod("+x", file.path)

      out = nil
      success = false
      begin
        Timeout.timeout(60) do
          out = `#{file.path} 2>&1`.strip
          success = $?.success?
        end
      rescue Timeout::Error
        out = "Command timed out"
        success = false
      end
      unless success
        message = <<~EOM
          Contents:

          #{contents.lines.map.with_index { |line, number| "  #{number.next} #{line.chomp}"}.join("\n") }

          Expected running script to succeed, but it did not

          Output:

            #{out}
        EOM

        raise message
      end
      out
    end

    it "Downloads a ruby binary" do
      Dir.mktmpdir do |dir|
        exec_with_bash_functions <<~EOM

          download_ruby "2.6.6" "#{dir}"
        EOM

        entries = Dir.entries(dir) - [".", ".."]

        expect(entries.sort).to eq(["bin", "include", "lib", "ruby.tgz", "share"])
      end
    end

    it "parses toml files" do
      out = exec_with_bash_functions <<~EOM
        ruby_version_from_toml "#{root_dir.join("buildpack.toml")}"
      EOM

      expect(out).to eq(RubyDetectVersion::DEFAULT)
    end

    it "downloads ruby to BUILDPACK_DIR vendor directory" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)

        exec_with_bash_functions(<<~EOM, stack: "heroku-18")
          BUILDPACK_DIR="#{dir}"
          download_ruby_version_to_buildpack_vendor "2.6.6"
        EOM

        expect(dir.entries.map(&:to_s)).to include("vendor")
        expect(dir.join("vendor").entries.map(&:to_s)).to include("ruby")
        expect(dir.join("vendor", "ruby").entries.map(&:to_s)).to include("heroku-18")
        expect(dir.join("vendor", "ruby", "heroku-18", "bin").entries.map(&:to_s)).to include("ruby")
      end
    end

    it "bootstraps ruby using toml file" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)

        FileUtils.cp(
          root_dir.join("buildpack.toml"), # From
          dir.join("buildpack.toml") # To
        )

        exec_with_bash_functions <<~EOM
          BUILDPACK_DIR="#{dir}"
          bootstrap_ruby_to_buildpack_dir
        EOM

        expect(dir.entries.map(&:to_s)).to include("vendor")
        expect(dir.join("vendor").entries.map(&:to_s)).to include("ruby")
        expect(dir.join("vendor", "ruby").entries.map(&:to_s)).to include("heroku-18")
        expect(dir.join("vendor", "ruby", "heroku-18", "bin").entries.map(&:to_s)).to include("ruby")
      end
    end

    it "outputs a node+ruby plan when a package.json is present" do
      Dir.mktmpdir do |dir|
        build_dir = Pathname(dir)

        build_dir.join("package.json").write "{}"

        plan_path = build_dir.join("plan.toml")
        exec_with_bash_functions <<~EOM
          # Stub out the call to `which node` so we can pretend it does NOT exist on the system
          which_node()
          {
            return 1
          }
          write_to_build_plan "#{plan_path}" "#{build_dir}"
        EOM

        toml = TOML.load(plan_path.read)

        expect(toml).to include(provides: [{name: "ruby"}])
        expect(toml).to include(requires: [{name: "node"}, {name: "ruby"}])
      end
    end

    it "does not output node when node is already installed" do
      Dir.mktmpdir do |dir|
        build_dir = Pathname(dir)

        build_dir.join("package.json").write "{}"

        plan_path = build_dir.join("plan.toml")
        plan_path = build_dir.join("plan.toml")
        exec_with_bash_functions <<~EOM
          # Stub out the call to `which node` so we can pretend it exists on the system
          which_node()
          {
            echo "foo"
            return 0
          }
          write_to_build_plan "#{plan_path}" "#{build_dir}"
        EOM

        toml = TOML.load(plan_path.read)

        expect(toml).to include(provides: [{name: "ruby"}])
        expect(toml).to include(requires: [{name: "ruby"}])
      end
    end

    it "detects if execjs is present" do
      Dir.mktmpdir do |dir|
        build_dir = Pathname(dir)
        package_json = build_dir.join("package.json")
        expect(package_json).to_not be_file

        build_dir.join("Gemfile.lock").write <<~EOM
          coffee-script (2.4.1)
            coffee-script-source
            execjs
        EOM

        exec_with_bash_functions <<~EOM
          BUILD_DIR="#{build_dir}"
          if needs_package_json "$BUILD_DIR"; then
            echo "{}" > "$BUILD_DIR/package.json"
          fi
        EOM

        expect(package_json).to be_file
        expect(package_json.read.strip).to eq("{}")

        build_dir.join("Gemfile.lock").write ""
        package_json.delete

        exec_with_bash_functions <<~EOM
          BUILD_DIR="#{build_dir}"
          if needs_package_json "$BUILD_DIR"; then
            echo "{}" > "$BUILD_DIR/package.json"
          fi
        EOM

        expect(package_json).to_not be_file
      end
    end

    it "outputs a ruby plan" do
      Dir.mktmpdir do |dir|
        build_dir = Pathname(dir)

        plan_path = build_dir.join("plan.toml")
        exec_with_bash_functions <<~EOM
          write_to_build_plan "#{plan_path}" "#{build_dir}"
        EOM

        toml = TOML.load(plan_path.read)

        expect(toml).to include(provides: [{name: "ruby"}])
        expect(toml).to include(requires: [{name: "ruby"}])
      end
    end
  end
end
