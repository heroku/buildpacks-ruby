require "pathname"
require "heroku_buildpack_ruby/toml"

module HerokuBuildpackRuby
  # This wraps setting ENV vars so we can record how we've changed
  # and use the values for exporting to layers or profile.d scripts
  #
  # We use this in legacy/v2 bin/compile to write env vars to export and .profile.d
  # We use this in CNB bin/build to write env vars to layers_dir/#{name}/env{.launch,.build}
  #
  # The main interface is the EnvProxy module. For example, to create a proxy for: ENV["PATH"]
  #
  #   app_dir = "/app"
  #   PATH_ENV = EnvProxy.path("PATH")
  #   PATH_ENV.prepend(ruby: "#{app_dir}/.heroku/ruby/path/bin")
  #
  # This will modify the current build path
  #
  #   puts ENV["PATH"] => "/app/.heroku/ruby/path/bin:/whatever/was/here/before"
  #
  # The proxy retains the modifications so they can be written to disk:
  #
  #   ruby_sh = Pathname.new(app_dir).join(".profile.d/ruby.sh")
  #   EnvProxy.export(ruby_sh)
  #   puts ruby_sh.read # => "export PATH="/app/.heroku/ruby/path/bin:$PATH"
  #
  # The key used when modifying a proxied env var can be used to write more granular layer info when using CNBs:
  #
  #   layers_dir = Pathname.new(layers_dir)
  #   EnvProxy.write_layers(layers_dir: layers_dir)
  #
  #   puts layers_dir.join("ruby/launch.env").entries # => "PATH"
  #   puts layers_dir.join("ruby/launch.env").read # => "/app/.heroku/ruby/path/bin"
  #   puts layers_dir.join("ruby.toml").read.lines.grep(/launch/) # => "launch = true\n"
  #
  # There are several ways to proxy an env var:
  #
  # - `PATH_ENV = EnvProxy.path("PATH")` # generates a path based env var that responds to `prepend`
  # - `BUNDLE_GEMFILE_ENV = EnvProxy.value("BUNDLE_GEMFILE") # generates a value based env var that responds to `set`
  #
  # To use a layer you have to first register it:
  #
  #   EnvProxy.register_layer(:ruby, build: true, launch: true, cache: false)
  #
  # For `EnvProxy.export` layer names are ignored as all values are output to the same file
  module EnvProxy
    extend Enumerable

    @env_array = []
    @registered_layers = {}

    # Used to add a valid layer to tracking
    #
    # Example:
    #
    #  EnvProxy.validate_layer!(:foo) #=> StandardError
    #
    #  EnvProxy.register_layer(:foo, build: true, cache: true, launch: true)
    #  EnvProxy.validate_layer!(:foo) # => Success
    def self.register_layer(name, build:, cache:, launch:)
      @registered_layers[name] = {build: build, cache: cache, launch: launch}
    end

    # Removes a given layer from tracking
    #
    # Example:
    #
    #   EnvProxy.delete_layer(:ruby)
    #
    #   EnvProxy.validate_layer!(:ruby) # => StandardError
    def self.delete_layer(name)
      @registered_layers.delete(name)
    end

    # Raises an error if a given layer is not tracked
    #
    # Example:
    #
    #  EnvProxy.validate_layer!(:foo) #=> StandardError
    #
    #  EnvProxy.register_layer(:foo, build: true, cache: true, launch: true)
    #  EnvProxy.validate_layer!(:foo) # => Success
    def self.validate_layer!(name)
      raise "Not a valid layer `#{name.inspect}` please use one of: #{@registered_layers.keys.inspect}" unless @registered_layers.key?(name)
    end

    # Creates an instance of EnvProxy and
    # tracks it globally.
    #
    # Use `value` over-writing the contents of other
    # env vars
    #
    # Example:
    #
    #   puts ENV["LOL"] #=> "haha"
    #   LOL_ENV = EnvProxy.value("LOL")
    #   LOL_ENV.set(ruby: "hehe")
    #
    #   puts ENV["LOL"] #=> "hehe"
    def self.value(key)
      value = EnvProxy::Value.new(key)
      @env_array << value
      value
    end

    # Creates an instance of EnvProxy and
    # tracks it globally.
    #
    # Use `path` for "PATH" like env vars that are prepended
    # to the current value and delimited by ":"
    #
    # Example:
    #
    #   puts ENV["LOL_PATH"] #=> "ha:ha"
    #   LOL_PATH_ENV = EnvProxy.path("LOL_PATH")
    #   LOL_PATH_ENV.prepend(ruby: "hehe")
    #
    #   puts ENV["LOL_PATH"] #=> "hehe:ha:ha"
    def self.path(key)
      value = EnvProxy::Array.new(key)
      @env_array << value
      value
    end

    # Interface to enumerable access to all
    # tracked env vars
    def self.each
      @env_array.each do |val|
        yield val
      end
    end

    # Deletes an env var from the current process and removes it from tracking
    def self.delete(env)
      ENV.delete(env.key)
      @env_array.delete(env)
    end

    # Top level interface for exporting env vars for profiled
    # and export file to other buildpacks
    def self.export(profile_d: , export: , app_dir: )
      profile_d_file = Pathname.new(profile_d).tap {|p| p.dirname.mkpath }
      export_file= Pathname.new(export)

      # Runtime, needs to escape app_dir with $HOME
      profile_d_file.open("w") do |f|
        @env_array.each do |env|
          f.puts env.to_export(replace_app_dir: app_dir)
        end
      end

      # Build time for other buildpacks, run in the same dir structure
      export_file.open("w") do |f|
        @env_array.each do |env|
          f.puts env.to_export
        end
      end
    end

    # Used to wrote all ENV vars to their correct env files
    # with cloud native buildpacks
    def self.write_layers(layers_dir: )
      @registered_layers.each do |name, config|
        layers_dir = Pathname.new(layers_dir)

        toml_file = layers_dir.join("#{name}.toml")
        toml_file.write(TOML::Dumper.new(config).to_s)

        @env_array.select {|env| env.touches_layer?(name) }.each do |env|
          env.write_layer(
            layers_dir: layers_dir,
            name: name,
          )
        end
      end
    end
  end

  class EnvProxy::Base
    attr_reader :key

    def initialize(key)
      @key = key

      @layer_env_hash = {}
    end

    def touches_layer?(name)
      @layer_env_hash.key?(name)
    end

    def layer_key
      key
    end

    def write_layer(layers_dir: , name:)
      layer = Pathname.new(layers_dir).join(name.to_s)
      launch_dir = layer.join("env.launch").tap(&:mkpath)
      build_dir = layer.join("env.build").tap(&:mkpath)

      build_dir.join(self.layer_key).open("w+") do |f|
        value = Array(@layer_env_hash[name]).join(":")
        f.write(value)
      end

      launch_dir.join(self.layer_key).open("w+") do |f|
        value = Array(@layer_env_hash[name]).join(":")
        f.write(value)
      end
    end

    private def layer_env_hash_without(app_dir: nil)
      return @layer_env_hash.dup if app_dir.nil?

      @layer_env_hash.each_with_object({}) do |(name, value), hash|
        hash[name] = Array(value).flatten.map {|v| v.gsub(/^#{app_dir}/, '$HOME') }.join(":")
      end
    end
  end

  # Used for setting a single value on an env var
  #
  # Example:
  #
  #   puts ENV["LOL"] #=> "haha"
  #   LOL_ENV = EnvProxy.value("LOL")
  #   LOL_ENV.set(ruby: "hehe")
  #
  #   puts ENV["LOL"] #=> "hehe"
  #
  class EnvProxy::Value < EnvProxy::Base
    def layer_key
      "#{key}.override"
    end

    def set(layer_env = {})
      if layer_env.values.uniq.count > 1
        raise "You've tried setting assigning the same ENV var to different values #{layer_env.values}"
      end
      @layer_env_hash.merge!(layer_env)

      value = layer_env.values.first.to_s

      @layer_env_hash.keys.each do |k|
        EnvProxy.validate_layer!(k)
        @layer_env_hash[k] = value
      end

      ENV[@key] = value
    end

    def to_export(replace_app_dir: nil)
      layer_env_hash_without(app_dir: replace_app_dir).map do |name, value|
        %Q{export #{key}="#{value}"}
      end.join("\n")
    end
  end


  # Used for prepending a value to a path based env var
  #
  # Example:
  #
  #   puts ENV["LOL_PATH"] #=> "ha:ha"
  #   LOL_PATH_ENV = EnvProxy.path("LOL_PATH")
  #   LOL_PATH_ENV.prepend(ruby: "hehe")
  #
  #   puts ENV["LOL_PATH"] #=> "hehe:ha:ha"
  class EnvProxy::Array < EnvProxy::Base
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

    def to_export(replace_app_dir: nil)
      layer_env_hash_without(app_dir: replace_app_dir).map do |name, array|
        value = [array, "$#{key}"].join(":")
        %Q{export #{key}="#{value}"}
      end.join("\n")
    end
  end

  PATH_ENV = EnvProxy.path("PATH")
  GEM_PATH_ENV = EnvProxy.path("GEM_PATH")
  BUNDLE_GEMFILE_ENV = EnvProxy.value("BUNDLE_GEMFILE")
end
