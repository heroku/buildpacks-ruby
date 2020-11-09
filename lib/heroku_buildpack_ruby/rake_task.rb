# frozen_string_literal: true

module HerokuBuildpackRuby

  # A wrapper for running rake tasks
  #
  # Example:
  #
  #   task = RakeTask.new("assets:precompile", stream: UserComms::CNB.new)
  #   task.call
  #   task.success? => true
  #   task.out => "Writing /app/public/assets/manifest-d0ff5974b6aa52cf562bea5921840c032a860a91a3512f7fe8f768f6bbe005f6.js \n #..."
  class RakeTask
    attr_reader :out

    def initialize(task, stream: nil)
      @bash_task = Bash.new("rake #{task}")
      @stream = stream || StringIO.new
      @called = false
      @status = nil
      @out = String.new
    end

    def call
      @called = true

      @bash_task.stream do |lines|
        @out << lines
        @stream.puts lines
      end
      @status = $?.success?

      self
    end

    def success?
      raise "must call" unless @called
      @status
    end

    def fail?
      !success?
    end
  end
end
