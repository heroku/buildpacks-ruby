require_relative "base.rb"

module HerokuBuildpackRuby
  # Used for prepending a value to a path based env var
  #
  # Example:
  #
  #   puts ENV["LOL_PATH"] #=> "ha:ha"
  #   LOL_PATH_ENV = EnvProxy.path("LOL_PATH")
  #   LOL_PATH_ENV.prepend(ruby: "hehe")
  #
  #   puts ENV["LOL_PATH"] #=> "hehe:ha:ha"
  #   puts LOL_PATH_ENV.to_export # => 'export LOL_PATH="hehe:$LOL_PATH"'
  #
  # Receives the following interfaces from the super class:
  #
  #   attr_reader :key
  #
  #   def initialize(key)
  #   def write_layer(layers_dir: )
  #   def write_exports(profile_d_path: , export_path: , app_dir: )
  #   def to_export(replace: "", with: "")
  class EnvProxy::Prepend < EnvProxy::Base
    # CNB filename for exports, prepend value https://github.com/buildpacks/spec/blob/main/buildpack.md#prepend
    def layer_env_type
      :prepend
    end

    # Main method for changing the value of the env var
    def prepend(layer_env = {})
      all_values = []
      layer_env.each do |layer_name, value|
        EnvProxy.validate_layer!(layer_name)

        value = Array(value).map(&:to_s)
        all_values << value

        @layer_env_hash[layer_name] ||= []
        @layer_env_hash[layer_name].prepend(value)
      end

      ENV[@key] = [all_values, ENV[key]].compact.join(":")
    end

    # Implement interface used by `for_export()` method to write profile_d and export files
    # outputs a singular value that has contents of all layers
    private def value_for_export(replace: "", with: "")
      values = @layer_env_hash.values.reverse.flatten.map {|v| v.sub(/^#{replace}/, with) }
      [values, "$#{key}"].join(":")
    end
  end
end
