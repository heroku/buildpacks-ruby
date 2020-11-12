# frozen_string_literal: true

require_relative "rake_task.rb"

module HerokuBuildpackRuby
  # Determines what rake tasks are available to a given app
  #
  # Example:
  #
  #   rake = RakeDetect.new(app_dir: Dir.pwd, has_rake_gem: true).call
  #   rake.detect?("assets:precompile") #=> true
  #   rake.detect?("doesnotexist") #=> false
  class RakeDetect
    RAKEFILE_NAMES = ["Rakefile", "rakefile", "rakefile.rb", "Rakefile.rb"].freeze

    private; attr_reader :error_if_detect_fails ; public

    def initialize(app_dir: , has_rake_gem: , error_if_detect_fails: false, user_comms: UserComms::Null.new)
      @app_dir = app_dir
      @user_comms = user_comms
      @detect_task = RakeTask.new("-P --trace", stream: false)
      @error_if_detect_fails = error_if_detect_fails

      @has_rake_gem = has_rake_gem
      @rakefile = (Pathname(app_dir).entries.map(&:to_s) & RAKEFILE_NAMES).first
    end

    def call
      @user_comms.topic("Detecting rake tasks")

      warn_no_rake_gem and return self if !has_rake_gem?
      warn_no_rakefile and return self if !rakefile?

      @detect_task.call

      detection_failed if @detect_task.fail?

      self
    end

    private def warn_no_rake_gem
      warn_or_error(
        title: "No rake gem",
        body: "No `rake` gem in the Gemfile.lock, skipping rake detection"
      )
    end

    private def warn_no_rakefile
      warn_or_error(
        title: "No Rakefile found",
        body: <<~EOM
          No Rakefile found in app directory, skipping rake detection

          Expected: #{RAKEFILE_NAMES.join(', ')}
          Found: #{@app_dir.entries.select(&:file?).join(', ')}
        EOM
      )
    end

    private def detection_failed
      command = String.new("bundle exec rake -P")
      command.prepend("RAILS_ENV=#{ENV['RAILS_ENV']} ") if ENV.key?("RAILS_ENV")

      warn_or_error(
        title: "Rake task detection failed",
        body: <<~EOM
          Ensure you can run `$ #{command}` against your app
          using the production group of your Gemfile.

          #{@detect_task.out}
        EOM
      )
    end

    private def warn_or_error(title:, body:)
      if error_if_detect_fails
        @user_comms.error(title: title, body: body)
      else
        @user_comms.warn_now(title: title, body: body)
      end
    end

    def detect?(name)
      @detect_task.out.match?(/\s#{name}/)
    end

    def loaded?
      has_rake_gem? && rakefile? && @detect_task.success?
    end

    def rakefile?
      @rakefile
    end

    def has_rake_gem?
      @has_rake_gem
    end
  end
end

