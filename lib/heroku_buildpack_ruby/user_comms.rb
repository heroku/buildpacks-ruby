# frozen_string_literal: true

module HerokuBuildpackRuby
  class UserComms; end

  class UserComms::Base
    attr_reader :io

    def initialize(io = $stdout)
      @io = io
      @io.sync = true

      @warnings = []
    end

    def close
      @warnings.each do |kwargs|
        warn_now(**kwargs)
      end
      self
    end

    def warn_later(title: , body:, link: nil)
      @warnings << {title: title, body: body, link: link}
    end

    def error(title: , body: , link: nil)
      raise BuildpackErrorNoBacktrace.new(title: title, body: body, link: link)
    end
  end

  # This class is used to format output for the user targeted at v2/legacy (bin/compile) mode
  #
  # Example:
  #
  #   stringio = StringIO.new
  #   user_output = UserOutput::V2.new(stringio)
  #   user_output.topic("hello there")
  #
  #   puts stringio.string # => "-----> hello there\n"
  class UserComms::V2 < UserComms::Base
    def topic(message) # status
      io.puts "-----> #{message}"
    end

    def info(message = "\n")
      message.to_s.each_line do |line|
        string = +""
        string << "       #{line}" unless line.strip.empty?
        string << "\n" unless string.end_with?("\n")
        io.print string
      end
    end

    def warn_now(title:, body: , link: nil)
      banner "Warning: #{title}"
      info
      info(body)
      return unless link
      info
      info("Link: #{link}")
    end

    def notice(message)
      banner "Notice: #{message}", color: "\033[1;38;2;64;143;236m"
    end

    def print_error_obj(error_obj)
      title = error_obj.title
      body = error_obj.body
      link = error_obj.link
      color = "\e[1m\e[31m" # bold red
      no_color = "\033[0m"

      banner "Error: #{title}", color: "\033[1;38;2;0;0;0;48;2;214;65;65m"

      info
      info   "#{color}!#{no_color}"
      body.split("\n").each do |line|
        info "#{color}!  #{line.strip}#{no_color}"
      end
      info   "#{color}! Link: #{link}#{no_color}" if link
      info   "#{color}!#{no_color}"
    end

    private def banner(message, color: "", no_color: "\033[0m")
      io.puts
      io.puts("       #{color}## #{message.strip}#{no_color}")
    end
  end

  # Like V2 output, but meant for the CNB interface
  class UserComms::CNB < UserComms::Base
    def topic(message) # Aka "status"
      banner message, color: "\033[1;38;2;157;112;208m"
    end

    # The default
    def info(message = "\n")
      message.to_s.each_line do |line|
        string = +"[INFO]"
        string << " #{line}" if !line.strip.empty?
        string << "\n" unless string.end_with?("\n")
        io.print string
      end
    end

    def print_error_obj(error_obj)
      banner "Error: #{error_obj.title}", color: "\033[1;38;2;0;0;0;48;2;214;65;65m"
      info(error_obj.body)

      return unless error_obj.link
      info
      info("Link: #{error_ob.link}")
    end

    def warn_now(title:, body:, link: nil)
      banner "Warning: #{title}", color: "\033[1;38;2;0;0;0;48;2;250;159;71m"
      info(body)

      return unless link
      info
      info("Link: #{link}")
    end

    def notice(message)
      banner "Notice: #{message}", color: "\033[1;38;2;64;143;236m"
    end

    private def banner(message, color: "", no_color: "\033[0m")
      io.puts
      io.puts("#{color}[#{message.strip}]#{no_color}")
    end
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
