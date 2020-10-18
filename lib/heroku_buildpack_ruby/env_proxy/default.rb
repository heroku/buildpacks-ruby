require_relative "base.rb"

module HerokuBuildpackRuby
  # Used for setting a single value on an env var
  #
  # Example:
  #
  #   puts ENV["LOL"] #=> "haha"
  #
  #   LOL_ENV = EnvProxy.default("LOL")
  #   LOL_ENV.set_default(ruby: "hehe")
  #
  #   puts ENV["LOL"] #=> "haha"
  #
  # Note: The output did NOT change since there WAS already a defined value
  #
  #   puts ENV.key?("NOPE") #=> false
  #
  #   NOPE_ENV = EnvProxy.default("NOPE")
  #   NOPE_ENV.set_default(ruby: "nopenopenope")
  #
  #   puts ENV["NOPE"] #=> "nopenopenope"
  #
  # Note: The output DID change since there was NOT already a defined value
  #
  # Receives the following interfaces from the super class:
  #
  #   attr_reader :key
  #
  #   def initialize(key)
  #   def value
  #   def to_env
  #   def write_layer(layers_dir: )
  #   def write_exports(profile_d_path: , export_path: , app_dir: )
  #   def to_export(replace: "", with: "")
  class EnvProxy::Default < EnvProxy::Base
    # Tells CNB to clobber any existing env vars with the same
    # key. https://github.com/buildpacks/spec/blob/main/buildpack.md#override
    def layer_env_type
      :default
    end

    # Main method for changing the value of the env var
    def set_default(layer_env = {})
      @layer_env_hash.merge!(layer_env)

      validate!

      value = layer_env.values.first.to_s

      @layer_env_hash.keys.each do |k|
        EnvProxy.validate_layer!(k)
        @layer_env_hash[k] = value
      end

      ENV[@key] ||= value

      self
    end

    # Sets the env var without saving it for runtime/launch
    def set_without_record(value)
      ENV[key] = value

      self
    end

    # Implement interface used by `for_export()` method to write profile_d and export files
    # outputs a singular value that has contents of all layers
    #
    # We also wrap the value in the bash logic for defaulting to a user supplied value
    private def value_for_export(replace: "", with: "")
      value = @layer_env_hash.values.flatten.map {|v| v.sub(/^#{replace}/, with) }.first
      return "${#{key}:-#{value}}"
    end

    # Since this class represents a singular value, all layer values must match
    # or we get undefined behavior based on load order
    private def validate!
      values = @layer_env_hash.values.map(&:to_s)
      values.uniq!
      if values.count > 1
        raise "You cannot set the same ENV var (#{key}) to different values values: #{values}, full_hash: #{@env_layer_hash.inspect}"
      end
    end
  end
end
