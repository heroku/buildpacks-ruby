# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "This buildpack" do
    describe "jruby" do
      Hatchet::Runner.new("default_ruby").tap do |app|
        app.before_deploy do
          dir = Pathname(Dir.pwd)
          dir.join("Gemfile").write <<~EOM
            source "https://rubygems.org"

            ruby '2.5.7', engine: 'jruby', engine_version: '9.2.13.0'
          EOM
          dir.join("Gemfile.lock").write <<~EOM
            GEM
              remote: https://rubygems.org/
              specs:

            PLATFORMS
              java

            RUBY VERSION
               ruby 2.5.7p001 (jruby 9.2.13.0)

            DEPENDENCIES
          EOM
          dir.join("system.properties").write("java.runtime.version=1.7")
        end
        app.deploy do
          puts app.output
        end
      end
    end

    it "user env compile" do
      Hatchet::Runner.new("default_ruby", config: {"BUNDLE_WITHOUT": "periwinkle"}).tap do |app|
        app.before_deploy do
        end
        app.deploy do
          expect(app.output).to include(%Q{BUNDLE_WITHOUT="periwinkle"})
          expect(app.output).to match("Installing bundler 1.")
        end
      end
    end

    it "getting started guide" do
      Hatchet::Runner.new("ruby-getting-started").deploy do |app|
        # TODO: Remove asset fragment cache before runtime for reduced slug size
        # expect(app.run("ls tmp/cache/assets")).to_not match("sprockets")
      end
    end

    it "bundler 1.x" do
      Hatchet::Runner.new("default_ruby").tap do |app|
        app.before_deploy do
          Pathname(Dir.pwd)
            .join("Gemfile.lock")
            .write("BUNDLED WITH\n   1.1.0", mode: "a")
        end
        app.deploy do
          # Test Bundler 1.x
          expect(app.output).to match("Installing bundler 1.")
        end
      end
    end

    it "has its own tests" do
      skip("Must set HATCHET_EXPENSIVE_MODE") unless ENV["HATCHET_EXPENSIVE_MODE"]

      Hatchet::Runner.new("default_ruby", run_multi: true).tap do |app|
        app.before_deploy do
          # TODO default process types
          Pathname(Dir.pwd).join("Procfile").write <<~EOM
            web: # No-op, needed so we can scale up for run_multi
          EOM
          Pathname(Dir.pwd)
            .join("Gemfile.lock")
            .write("BUNDLED WITH\n   2.1.4", mode: "a")
        end
        app.deploy do
          # Test bundler 2.x
          expect(app.output).to match("Installing bundler 2.")

          # Test deploy succeeded
          expect(app.output).to match("deployed to Heroku")

          # Test dependencies installed
          expect(app.output).to match("Installing rake")

          # Test default ruby is installed
          app.run_multi("ruby -v") do |out, status|
            expect(out).to match(RubyDetectVersion::DEFAULT)
            expect(status.success?).to be_truthy
          end

          # Test that the system path isn't clobbered
          app.run_multi("which bash") do |out, status|
            expect(out.strip).to eq("/bin/bash")
            expect(status.success?).to be_truthy
          end

          # Verify gem installation location does not change
          # and binstubs are on the path
          app.run_multi("which -a rake") do |out, status|
            # Gem rake version
            expect(out).to include("/app/.heroku/ruby/gems/bin/rake")
            expect(status.success?).to be_truthy
          end

          # Test deploys twice without error
          app.commit!
          app.push!

          # Test uses cache after re-deploy
          expect(app.output).to match("Using rake")
        end
      end
    end

    it "installs node and yarn and calls assets:precompile" do
      skip("Must set HATCHET_EXPENSIVE_MODE") unless ENV["HATCHET_EXPENSIVE_MODE"]

      Hatchet::Runner.new("minimal_webpacker", run_multi: true).tap do |app|
        app.before_deploy do
          Pathname(Dir.pwd).join("Procfile").write <<~EOM
            web: # No-op, needed so we can scale up for run_multi
          EOM
        end
        app.deploy do
          # This output comes from the heroku/nodejs buildpack
          expect(app.output).to include("installing yarn")

          # This output comes from the Ruby buildpack
          expect(app.output).to include("Installing rake")

          # This output comes from the contents of the Rakefile
          # https://github.com/sharpstone/minimal_webpacker/blob/master/Rakefile
          expect(app.output).to include("THE TASK ASSETS:PRECOMPILE WAS CALLED")
          expect(app.output).to include("THE TASK ASSETS:CLEAN WAS CALLED")

          app.run_multi("which node") do |out, status|
            expect(out.strip).to_not be_empty
            expect(status.success?).to be_truthy
          end

          app.run_multi("which yarn") do |out, status|
            expect(out.strip).to_not be_empty
            expect(status.success?).to be_truthy
          end
        end
      end
    end

    # https://github.com/heroku/heroku-buildpack-ruby/pull/124
    it "nokogiri should use the system libxml2" do
      Hatchet::Runner.new("default_ruby").tap do |app|
        app.before_deploy do
          dir = Pathname(Dir.pwd)
          dir.join("Gemfile").write <<~EOM
            source "https://rubygems.org"

            gem "nokogiri", "1.6.0"
          EOM

          dir.join("Gemfile.lock").write <<~EOM
            GEM
              remote: https://rubygems.org/
              specs:
                mini_portile (0.5.1)
                nokogiri (1.6.0)
                  mini_portile (~> 0.5.0)

            PLATFORMS
              ruby

            DEPENDENCIES
              nokogiri (= 1.6.0)
          EOM
        end

        app.deploy do
          expect(app.output).to match("nokogiri")

          expect(
            app.run(%q{ruby -rnokogiri -e 'puts "Using system libxml2: #{Nokogiri::VersionInfo.new.libxml2_using_system?}"'})
          ).to match("Using system libxml2: true")
          expect($?.success?).to be_truthy
        end
      end
    end
  end
end
