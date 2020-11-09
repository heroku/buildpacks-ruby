# frozen_string_literal: true

module HerokuBuildpackRuby
  # Copy the cache from one location to another
  #
  # With CNB layers dir is available at runtime with V2 cache contents must be coppied
  #
  # Example:
  #
  #   cache_dir = Pathname("/tmp/cache")
  #   dest_dir = Pathname("/tmp/foo")

  #   puts cache_dir.join("hello.txt").exist? # => false
  #
  #   CacheCopy.new(cache_dir: cache_dir, dest_dir: dest_dir).call do
  #     dest_dir.join("hello.txt").write "hello world")
  #   end
  #
  #   puts cache_dir.join("hello.txt").exist? # => true
  #
  #   different_dest_dir = Pathname("/tmp/bar")
  #   puts different_dest_dir.join("hello.txt").exist? # => false
  #
  #   CacheCopy.new(cache_dir: cache_dir, dest_dir: different_dest_dir).call do
  #     puts different_dest_dir.join("hello.txt").exist? # => true
  #   end
  class CacheCopy
    def initialize(cache_dir: , dest_dir:)
      @cache_dir = Pathname(cache_dir).tap(&:mkpath)
      @dest_dir = Pathname(dest_dir).tap(&:mkpath)
    end

    def call
      raise "nope" unless block_given?

      # https://ruby-doc.org/stdlib-2.7.2/libdoc/fileutils/rdoc/FileUtils.html#method-c-cp_r
      FileUtils.cp_r(@cache_dir.glob("*"), @dest_dir)
      yield @dest_dir
      FileUtils.cp_r(@dest_dir.glob("*"), @cache_dir)
    end
  end
end
