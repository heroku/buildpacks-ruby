# frozen_string_literal: true

require_relative '../spec_helper'

module HerokuBuildpackRuby
  RSpec.describe "Cloud Native Buildpack" do
    it "jruby" do
      Cutlass::App.new(
        "default_ruby",
        buildpacks: ["heroku/jvm@0.1.3", :default]
      ).transaction do |app|
        app.tmpdir.join("Gemfile").write <<~EOM
          source "https://rubygems.org"

          ruby '2.5.7', engine: 'jruby', engine_version: '9.2.13.0'
        EOM

        app.tmpdir.join("Gemfile.lock").write <<~EOM
          GEM
            remote: https://rubygems.org/
            specs:

          PLATFORMS
            java

          RUBY VERSION
             ruby 2.5.7p001 (jruby 9.2.13.0)

          DEPENDENCIES
        EOM

        app.pack_build

        expect(app.stdout).to include("[Installing Java]")
        expect(app.stdout).to include("Using Ruby version: 2.5.7-jruby-9.2.13.0")
        expect(app.stdout).to include("Bundle complete")
      end
    end

    it "locally runs default_ruby app" do
      Cutlass::App.new("default_ruby").transaction do |app|
        app.pack_build

        expect(app.stdout).to include("Installing rake")

        app.run_multi("ruby -v") do |out|
          expect(out.stdout).to match(RubyDetectVersion::DEFAULT)
        end

        app.run_multi("bundle list") do |out|
          expect(out.stdout).to match("rack")
        end

        app.run_multi("gem list") do |out|
          expect(out.stdout).to match("rack")
        end

        app.run_multi(%Q{ruby -e "require 'rack'; puts 'done'"}) do |out|
          expect(out.stdout).to match("done")
        end

        # Test cache
        app.pack_build

        expect(app.stdout).to include("Using rake")
      end
    end

    it "installs node and yarn and calls assets:precompile" do
      Cutlass::App.new(
        "minimal_webpacker",
        buildpacks: ["heroku/nodejs", :default]
      ).transaction do |app|
        app.pack_build

        expect(app.stdout).to include("Installing rake")
        expect(app.stdout).to include("Installing yarn")

        # This output comes from the contents of the Rakefile
        # https://github.com/sharpstone/minimal_webpacker/blob/master/Rakefile
        expect(app.stdout).to include("THE TASK ASSETS:PRECOMPILE WAS CALLED")
        expect(app.stdout).to include("THE TASK ASSETS:CLEAN WAS CALLED")

        app.run_multi("which node") do |result|
          expect(result.stdout.strip).to_not be_empty
          expect(result.success?).to be_truthy
        end

        app.run_multi("which yarn") do |result|
          expect(result.stdout.strip).to_not be_empty
          expect(result.success?).to be_truthy
        end
      end
    end

    it "Respects user config vars" do
      Cutlass::App.new(
        "default_ruby",
        config: { "BUNDLE_WITHOUT": "periwinkle" }
      ).transaction do |app|
        app.pack_build do |result|
          expect(result.stdout).to include(%Q{BUNDLE_WITHOUT="periwinkle"})
        end
      end
    end

    it "rails getting started guide" do
      # TODO, detect exectjs at detect time and remove heroku/nodejs from buildpacks path

      Cutlass::App.new(
        "ruby-getting-started",
        buildpacks: ["heroku/nodejs", :default]
      ).transaction do |app|
        app.pack_build

        app.run_multi("rails runner 'puts ENV[%Q{RAILS_SERVE_STATIC_FILES}].present?'") do |result|
          expect(result.stdout).to match(/true/)
          expect(result.success?).to be_truthy
        end
      end
    end
  end
end
