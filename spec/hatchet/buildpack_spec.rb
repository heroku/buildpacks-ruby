require_relative "../spec_helper.rb"

RSpec.describe "This buildpack" do
  it "has its own tests" do
    Hatchet::Runner.new("default_ruby").tap do |app|
      app.before_deploy do
      end
      app.deploy do
        # Assert the behavior you desire here
        expect(app.output).to match("deployed to Heroku")
        expect(app.output).to match("Installing rake")
        expect(app.run("ruby -v")).to match("2.6.6")

        # Test that the system path isn't clobbered
        expect(app.run("which bash").strip).to eq("/bin/bash")

        app.commit!
        app.push!

        expect(app.output).to match("Using rake")
      end
    end
  end

  it "installs node and yarn" do
    Hatchet::Runner.new("minimal_webpacker").tap do |app|
      app.before_deploy do
      end
      app.deploy do
        expect(app.output).to include("Installing rake")

        expect(app.output).to include("installing node")
        expect(app.output).to include("installing yarn")
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
