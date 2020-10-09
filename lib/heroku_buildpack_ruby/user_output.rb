module HerokuBuildpackRuby
  class UserOutput; end
  # This class is used to format output for the user targeted at v2/legacy (bin/compile) mode
  #
  # Example:
  #
  #   stringio = StringIO.new
  #   user_output = UserOutput::V2.new(stringio)
  #   user_output.topic("hello there")
  #
  #   puts stringio.string # => "-----> hello there\n"
  class UserOutput::V2
    attr_reader :io

    def initialize(io = $stdout)
      @io = io
      @io.sync = true
    end

    def topic(str)
      io.puts "-----> #{str}"
    end
  end

  # Like V2 output, but meant for the CNB interface
  class UserOutput::CNB < UserOutput::V2
  end
end
