require_relative "../spec_helper.rb"

RSpec.describe "This buildpack" do
  it "has its own tests" do
    # Specify where you want your buildpack to go using :default
    # To deploy a different app modify the hatchet.json or
    # commit an app to your source control and use a path
    # instead of "default_ruby" here
    Hatchet::Runner.new("default_ruby").tap do |app|
      app.before_deploy do
      end
      app.deploy do
        # Assert the behavior you desire here
        expect(app.output).to match("deployed to Heroku")
        expect(app.run("ruby -v")).to match("2.6.6")
      end
    end
  end
end
