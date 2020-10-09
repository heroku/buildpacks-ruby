require "shellwords"

module HerokuBuildpackRuby
  # Shell out with style
  #
  # Example:
  #
  #   Bash.new("echo 'hi'").run # => "hi\n"
  #
  # Call run with a bang (!) to check for errors
  #
  #   Bash.new("this command does not exist").run! # => <# BashError>
  class Bash
    class BashError < StandardError; end

    def initialize(raw_command, max_attempts: 0, redirect: "2>&1")
      @raw_command = raw_command
      @max_attempts = max_attempts
      @redirect = redirect

      @command = build_command
    end

    def run
      `#{@command}`
    end

    def run!
      out = run
      raise BashError, "Command: '#{@command}' failed unexpectedly:\n#{out}" unless $?.success?
      out
    end

    # TODO  bash shellscaping fun-ness
    private def build_command
      "/usr/bin/env bash -c #{@raw_command.shellescape} #{@redirect} "
    end
  end
end
# TODO: max_attempts
# TODO: User env support
