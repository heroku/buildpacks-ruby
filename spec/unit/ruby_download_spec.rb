require_relative "../spec_helper.rb"

RSpec.describe "download ruby" do
  it "downloads a ruby" do
    Dir.mktmpdir do |dir|
      HerokuBuildpackRuby::RubyDownload.new(
        version: "2.7.2",
        stack: "heroku-18",
        install_dir: dir
      ).call

      entries = Dir.entries(dir) - [".", ".."]
      expect(entries).to include("bin")
      expect(entries).to include("include")
      expect(entries).to include("lib")
      expect(entries).to include("share")
    end
  end
end
