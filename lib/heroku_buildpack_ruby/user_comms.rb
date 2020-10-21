module HerokuBuildpackRuby
  class UserComms; end
  # This class is used to format output for the user targeted at v2/legacy (bin/compile) mode
  #
  # Example:
  #
  #   stringio = StringIO.new
  #   user_output = UserOutput::V2.new(stringio)
  #   user_output.topic("hello there")
  #
  #   puts stringio.string # => "-----> hello there\n"
  class UserComms::V2
    attr_reader :io

    def initialize(io = $stdout)
      @io = io
      @io.sync = true

      @warnings = []
    end

    def topic(message)
      io.puts "-----> #{message}"
    end

    def close
      @warnings.each do |message|
        warn_now(message)
      end
      self
    end

    def warn_later(message)
      @warnings << message
    end

    def warn_now(message)
      self.puts
      self.puts "## Warning"
      self.puts
      self.puts(message)
      self.puts
    end

    def error_and_exit(message)
      self.puts "\e[1m\e[31m" # Bold Red
      self.puts " !"
      message.split("\n").each do |line|
        io.puts " !     #{line.strip}"
      end
      self.puts " !\e[0m"
      exit(1)
    end

    def puts(message = "")
      message.to_s.each_line do |line|
        if line.end_with?("\n".freeze)
          io.print "       #{line}"
        else
          io.print "       #{line}\n"
        end
      end
    end
  end

  # Like V2 output, but meant for the CNB interface
  class UserComms::CNB < UserComms::V2
  end

  class UserComms::Null < UserComms::V2
    def initialize(io = StringIO.new)
      super
    end

    # Only available on UserComms::Null for testing
    def to_string
      io.string
    end
  end
end
