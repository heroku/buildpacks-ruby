module HerokuBuildpackRuby
  # It wraps version info and pretends to be a string
  #
  # Example:
  #
  #   v = RubyVersionInfo.new(version: "2.7.2")
  #   puts v.to_s # => "2.7.2"
  #
  #   v = RubyVersionInfo.new(
  #     version: "2.5.7",
  #     engine: :jruby,
  #     engine_version: "9.2.13.0"
  #   )
  #   puts v.to_s # => "2.5.7-jruby-9.2.13.0"
  #
  # There is strong coupling between this output and naming structure
  # of Ruby binaries on S3
  class RubyVersionInfo
    attr_reader :version, :engine, :engine_version
    def initialize(version: , engine: nil, engine_version: nil)
      @engine = engine
      @version = version
      @engine_version = engine_version
      @string = [version, engine, engine_version].compact.join("-")
    end

    def to_s
      @string
    end
  end
end
