# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "RubyVersionInfo" do
    it "acts like a string" do
      v = RubyVersionInfo.new(version: "2.7.2")
      expect("#{v}").to eq("2.7.2")
    end

    it "formats jruby" do
      v = RubyVersionInfo.new(
        engine: :jruby,
        version: "2.5.7",
        engine_version: "9.2.13.0"
      )
      expect("#{v}").to eq("2.5.7-jruby-9.2.13.0")
    end

    it "matches S3 format for jruby" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)

        v = RubyVersionInfo.new(
          engine: :jruby,
          version: "2.5.7",
          engine_version: "9.2.13.0"
        )
        download = RubyDownload.new(
          version: v,
          stack: "heroku-20",
          install_dir: dir
        )
        expect(download).to exist
      end
    end

    it "matches s3 format for ruby" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)

        v = RubyVersionInfo.new(
          version: "2.7.2",
        )
        download = RubyDownload.new(
          version: v,
          stack: "heroku-20",
          install_dir: dir
        )
        expect(download).to exist
      end
    end
  end
end
