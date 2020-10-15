require_relative "base.rb"

module HerokuBuildpackRuby
  # Used for setting a single value on an env var
  #
  # Example:
  #
  #   puts ENV["LOL"] #=> "haha"
  #
  #   LOL_ENV = EnvProxy.value("LOL")
  #   LOL_ENV.set(ruby: "hehe")
  #
  #   puts ENV["LOL"] #=> "hehe"
  #
  # Receives the following interfaces from the super class:
  #
  #   attr_reader :key
  #
  #   def initialize(key)
  #   def write_layer(layers_dir: )
  #   def write_exports(profile_d_path: , export_path: , app_dir: )
  #   def to_export(replace: "", with: "")
  class EnvProxy::Override < EnvProxy::Base
    # Tells CNB to clobber any existing env vars with the same
    # key. https://github.com/buildpacks/spec/blob/main/buildpack.md#override
    def layer_env_type
      :override
    end

    # Main method for changing the value of the env var
    def set(layer_env = {})
      @layer_env_hash.merge!(layer_env)

      validate!

      value = layer_env.values.first.to_s

      @layer_env_hash.keys.each do |k|
        EnvProxy.validate_layer!(k)
        @layer_env_hash[k] = value
      end

      ENV[@key] = value
    end

    # Implement interface used by `for_export()` method to write profile_d and export files
    # outputs a singular value that has contents of all layers
    private def value_for_export(replace: "", with: "")
      @layer_env_hash.values.flatten.map {|v| v.sub(/^#{replace}/, with) }.first
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
