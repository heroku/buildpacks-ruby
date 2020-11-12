# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "download ruby" do
    it "downloads a ruby" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)
        download = RubyDownload.new(
          version: "2.7.2",
          stack: "heroku-18",
          install_dir: dir
        )

        expect(download.exist?).to be_truthy

        download.call

        entries = dir.entries.map(&:to_s)
        expect(entries).to include("bin")
        expect(entries).to include("include")
        expect(entries).to include("lib")
        expect(entries).to include("share")
      end
    end
  end
end
