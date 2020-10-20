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
      @install_dir = Pathname.new(install_dir).tap(&:mkpath)
    end

    def call
      user_comms.topic("Using Ruby version: #{version}")

      CurlFetch.new(
        "ruby-#{version}.tgz",
        folder: stack,
        install_dir: install_dir
      ).fetch_untar
    end
  end
end
