# frozen_string_literal: true

require "pathname"
# require_relative "fetch_command"

module HerokuBuildpackRuby
  # Downloads a folder containing compiled ruby to the designated directory
  class RubyDownload
    private; attr_reader :user_comms, :version, :install_dir, :stack; public;

    def initialize(version: , install_dir: , stack: ENV["STACK"], user_comms: UserComms::Null.new)
      @stack = stack
      @version = version
      @user_comms = user_comms
      @install_dir = Pathname(install_dir).tap(&:mkpath)
      @fetcher = CurlFetch.new(
        "ruby-#{version}.tgz",
        folder: stack,
        install_dir: install_dir
      )

      raise "Must provide a stack #{@stack} to download a Ruby version" if @stack.to_s.empty?
    end

    def exist?
      @fetcher.exist?
    end
    alias :exists? :exist?

    def call
      user_comms.info("Using Ruby version: #{version}")
      @fetcher.fetch_untar
    end
  end
end
