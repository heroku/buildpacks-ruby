require "pathname"
# require_relative "fetch_command"

module HerokuBuildpackRuby
  # Downloads a folder containing compiled ruby to the designated directory
  class RubyDownload
    attr_reader :version, :install_dir, :stack

    def initialize(version: , install_dir: , stack: ENV["STACK"])
      @install_dir = Pathname.new(install_dir).tap(&:mkpath)
      @version = version
      @stack = stack
    end

    def call
      CurlFetch.new(
        "ruby-#{version}.tgz",
        folder: stack,
        install_dir: install_dir
      ).fetch_untar
    end
  end
end
