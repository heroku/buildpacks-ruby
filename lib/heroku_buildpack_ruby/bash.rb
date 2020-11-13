# frozen_string_literal: true

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

          Original:
            #{bash.raw_command}

          Escaped:
            #{bash.command_without_env}

          Out:
            "#{out}"
        EOM
      end
    end

    private; attr_reader :user_env; public
    attr_reader :command, :raw_command

    def initialize(raw_command, max_attempts: 0, redirect: "2>&1", user_env: UserEnv)
      @user_env = user_env
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

    def command_without_env
      return @command if !user_env || user_env.empty?

      @command.sub(user_env.to_shell, "<REDACTED>")
    end

    private def build_command
      array = []
      array << "/usr/bin/env"
      array << user_env.to_shell if user_env
      array << "bash -c"
      array << @raw_command.shellescape
      array << @redirect
      array.join(" ")
    end
  end
end
# TODO: max_attempts
# TODO: User env support
