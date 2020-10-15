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

    attr_reader :version, :gemfile_path, :buildpack_ruby_path, :bundler_path, :lockfile_path

    def initialize(gemfile_dir: , buildpack_ruby_path: , bundler_path: )
      @gemfile_dir = Pathname.new(gemfile_dir)
      @buildpack_ruby_path = Pathname.new(buildpack_ruby_path)
      @bundler_path = Pathname.new(bundler_path)
      @gemfile_path = Pathname.new(@gemfile_dir).join("Gemfile")
      @lockfile_path = Pathname.new(@gemfile_dir).join("Gemfile.lock")
    end

    def call
      out = bundler_output
      if (md = RUBY_VERSION_REGEX.match(out))
        @version = md[:ruby_version]
      else
        @version   = DEFAULT
      end
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
  end
end

