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
  #   PATH_ENV = EnvProxy.path("PATH")
  #
  # This will modify the current build path:
  #
  #   PATH_ENV.prepend(ruby: "/app/.heroku/ruby/path/bin")
  #   puts ENV["PATH"] => "/app/.heroku/ruby/path/bin:/whatever/was/here/before"
  #
  # The proxy retains modifications so they can be written to disk for various interfaces. Such as CNB and v2/legacy.
  # The key for the value represents the layer it will be written to. This example shows writing to a ruby layer:
  #
  #   PATH_ENV.prepend(ruby: "/app/.heroku/ruby/path/bin")
  #
  # Later when all layers are written to we can expect to see this environment variable set in the layers dir
  # via the `EnvProxy.write_layers` method:
  #
  #   layers_dir = Pathname.new(layers_dir)
  #   EnvProxy.write_layers(layers_dir: layers_dir)
  #
  #   puts layers_dir.join("ruby/launch.env").entries # => "PATH"
  #   puts layers_dir.join("ruby/launch.env/PATH").read # => "/app/.heroku/ruby/path/bin"
  #   puts layers_dir.join("ruby.toml").read.lines.grep(/launch/) # => "launch = true\n"
  #
  # > Note: this method also generates a `ruby.toml`. To configure the contents use `EnvProxy.register_layer`.
  #
  # To write to an export file such as profile.d script or a bash profile you can use the `EnvProxy.export_to` interface
  # which support v2/legacy:
  #
  #   ruby_sh = Pathname.new("/app").join(".profile.d/ruby.sh")
  #   export = Pathname.new(BUILDPACK_PATH).join("export")
  #
  #   EnvProxy.export_to(profile_d_path: ruby_sh, export_path: export_path, app_dir: "/app")
  #
  #   puts ruby_sh.read # => "export PATH="/app/.heroku/ruby/path/bin:$PATH"
  #
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
  # For `EnvProxy.export_to` the layer names are ignored as all values are output to the same file
  #
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
      value = EnvProxy::Override.new(key)
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
      value = EnvProxy::Prepend.new(key)
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
    def self.export(profile_d_path: , export_path: , app_dir: )
      profile_d_path = Pathname.new(profile_d_path).tap {|p| p.dirname.mkpath }
      export_path = Pathname.new(export_path)

      @env_array.each do |env|
        env.write_exports(
          app_dir: app_dir,
          profile_d_path: profile_d_path,
          export_path: export_path
        )
      end
    end

    # Used to wrote all ENV vars to their correct env files
    # with cloud native buildpacks
    def self.write_layers(layers_dir: )
      layers_dir = Pathname.new(layers_dir)

      @registered_layers.each do |name, config|
        contents = TOML::Dumper.new(config).to_s
        layers_dir.join("#{name}.toml").write(contents)
      end

      @env_array.each do |env|
        env.write_layer(layers_dir: layers_dir)
      end
    end
  end

  PATH_ENV = EnvProxy.path("PATH")
  GEM_PATH_ENV = EnvProxy.path("GEM_PATH")
  BUNDLE_GEMFILE_ENV = EnvProxy.value("BUNDLE_GEMFILE")
end

require_relative "env_proxy/prepend.rb"
require_relative "env_proxy/override.rb"
