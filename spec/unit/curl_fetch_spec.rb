# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "CurlFetch" do
    it "knows if binaries exist" do
      Dir.mktmpdir do |dir|
        fetcher = CurlFetch.new(
          "ruby-2.7.2.tgz",
          folder: "heroku-20",
          install_dir: dir
        )
        expect(fetcher.exist?).to be_truthy

        fetcher = CurlFetch.new(
          "nope-nope-nope.tgz",
          folder: "heroku-20",
          install_dir: dir
        )
        expect(fetcher.exist?).to be_falsey
      end
    end
  end
end
