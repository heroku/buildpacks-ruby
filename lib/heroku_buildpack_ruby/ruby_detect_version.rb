# frozen_string_literal: true

require "pathname"

module HerokuBuildpackRuby
  # Detects the app's Ruby version based off of Gemfile.lock contents or `bundle platform --ruby` output
  #
  # Since we don't know what version of Ruby the app needs we don't have it installed yet
  # we have to use another ruby version to execute bundler's `bundle platform --ruby`.
  #
  # The Ruby buildpack vendors a version of Ruby that can be used.
  #
  # Example:
  #
  #   ruby_detect = RubyDetectVersion.new(
  #     gemfile_dir: ".",
  #     bundler_path: `which bundle`.strip,,
  #     buildpack_ruby_path: `which ruby`.strip
  #   )
  #
  #   ruby_detect.call
  #   ruby_detect.version #=> "2.7.2"
  #
  # Metadata: [:ruby][:default_version]
  class RubyDetectVersion
    DEFAULT = "2.6.6".freeze

    # Matches sanitized output from bundler
    RUBY_VERSION_REGEX = %r{
      (?<ruby_version>\d+\.\d+\.\d+){0}
      (?<patchlevel>p-?\d+){0}
      (?<engine>\w+){0}
      (?<engine_version>.+){0}

      ruby-\g<ruby_version>(-\g<patchlevel>)?(-\g<engine>-\g<engine_version>)?
    }x

    # Matches raw lockfile regex
    RUBY_GEMFILE_LOCK_REGEX = /^RUBY VERSION$(\r?\n)   (?<raw_bundler_output>ruby .*$)/

    private; attr_reader :user_comms, :ruby_metadata, :default_version, :gemfile_path, :buildpack_ruby_path, :bundler_path, :lockfile_path; public
    public; attr_reader :version

    def initialize(gemfile_dir: , buildpack_ruby_path: , bundler_path: , metadata: Metadata::Null.new, default_version: DEFAULT, user_comms: UserComms::Null.new)
      gemfile_dir = Pathname(gemfile_dir)
      @bundler_path = Pathname(bundler_path)
      @gemfile_path = gemfile_dir.join("Gemfile")
      @lockfile_path = gemfile_dir.join("Gemfile.lock")
      @buildpack_ruby_path = Pathname(buildpack_ruby_path)

      @user_comms = user_comms
      @ruby_metadata = metadata.layer(:ruby)
      @default_version = default_version
    end

    def call
      out = bundler_output
      if (md = RUBY_VERSION_REGEX.match(out))
        @version = md[:ruby_version]
      else
        @version = ruby_metadata.fetch(:default_version) { default_version }
        warn_default_ruby
      end
      self
    end

    private def bundler_output
      output = bundler_output_from_lockfile!
      output ||= bundler_output_from_shell!

      output.strip.sub('(', '').sub(')', '').sub(/(p-?\d+)/, ' \1').split.join('-')
    end

    # TODO nicer error handling
    def bundler_output_from_shell!
      # Run `bundle platform --ruby` but use the bootstrapped version of ruby for the buildpack
      # To do this we directly call the bootstrapped binary path then directly pass it the bootstrapped ruby
      output = Bash.new(%Q{BUNDLE_GEMFILE="#{gemfile_path}" #{buildpack_ruby_path} #{bundler_path} platform --ruby}).run!
      output = output.strip.lines.last

      if output.match(/No ruby version specified/)
        ""
      else
        output
      end
    end

    def bundler_output_from_lockfile!
      md = RUBY_GEMFILE_LOCK_REGEX.match(lockfile_path.read)
      md[:raw_bundler_output] if md
    end

    private def warn_default_ruby
      warning = <<~WARNING
        You have not declared a Ruby version in your Gemfile.

        To declare a Ruby version add this line to your Gemfile:

        ```
        ruby "#{default_version}"
        ```

        For more information see:
          https://devcenter.heroku.com/articles/ruby-versions
      WARNING

      user_comms.warn_later(warning)
    end
  end
end

