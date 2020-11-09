# frozen_string_literal: true

require_relative "../spec_helper.rb"

RSpec.describe "cache copy" do
  it "" do
    Dir.mktmpdir do |dir|
      dir = Pathname(dir)
      dest_dir = dir.join("foo").tap(&:mkpath)
      cache_dir = dir.join("cache").tap(&:mkpath)

      expect(dest_dir).to be_empty
      HerokuBuildpackRuby::CacheCopy.new(cache_dir: cache_dir, dest_dir: dest_dir).call do |block_value|
        dest_dir.join("hello.txt").write "hello world"

        expect(dest_dir.entries.map(&:to_s)).to include("hello.txt")
        expect(dest_dir).to eq(block_value)
      end
      expect(dest_dir.entries.map(&:to_s)).to include("hello.txt")
      expect(cache_dir.entries.map(&:to_s)).to include("hello.txt")

      different_dest_dir = dir.join("bar").tap(&:mkpath)

      expect(different_dest_dir).to be_empty
      HerokuBuildpackRuby::CacheCopy.new(cache_dir: cache_dir, dest_dir: different_dest_dir).call do
        expect(different_dest_dir.entries.map(&:to_s)).to include("hello.txt")
      end
    end
  end
end
