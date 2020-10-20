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
    class BashError < StandardError
      def initialize(bash, out: )
        super <<~EOM
          Bash command failed

          Original: #{bash.raw_command}
          Escaped: #{bash.command}
          Out: "#{out}"
        EOM
      end
    end

    attr_reader :command, :raw_command

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
      raise BashError.new(self, out: out)  unless $?.success?
      out
    end

    def stream
      out = String.new
      IO.popen(@command) do |io|
        until io.eof?
          buffer = io.gets
          out << buffer

          yield buffer if block_given?
        end
      end

      out
    end

    private def build_command
      "/usr/bin/env bash -c #{@raw_command.shellescape} #{@redirect} "
    end
  end
end
# TODO: max_attempts
# TODO: User env support
