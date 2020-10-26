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
end
