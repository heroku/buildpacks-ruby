module HerokuBuildpackRuby
  # Base class for EnvProxy types
  #
  # Docs here are for implementing your own subclass
  #
  # If you subclass you'll need to implement:
  #
  # - (set|prepend|<user-defined>): A method for importing data. Such as `set` for values and `prepend` for paths. You pick a method
  #   name to match the semantics you are expecting for the proxy type.
  # - `private def value_for_export`: This method converts ALL layer values to a singular value used for exporting to v2 interface
  #   this will be called once without arguments for writing an export file that sets env vars for later buildpacks
  #   as well as being called once with `replace:` and `with:` for writing an export file that writes to profile_d
  #   file for runtime execution
  # - `def layer_env_type`: A method that returns one of: [:prepend, :append, :override, :default] corresponding to the behavior
  #   of the environment variable coresponding to the layer https://github.com/buildpacks/spec/blob/main/buildpack.md#environment-variable-modification-rules
  #
  # Internal structure:
  #
  # - @layer_env_hash: Is a hash, the keys represent names of layers for CNB, the values represent values where env modifications
  #   can be recorded. Different subclasses may have different value representations. For instance the PATH proxy is represented by arrays
  #   while a value env like BUNDLE_GEMFILE proxy is represented by a single value.
  #
  #   Example: {ruby: "Gemfile"} in this case the "ruby" layer would have a value of "Gemfile" for the initialized key
  #
  # Provides the following interfaces to subclasses:
  #
  #   attr_reader :key
  #
  #   def initialize(key)
  #   def write_layer(layers_dir: )
  #   def write_exports(profile_d_path: , export_path: , app_dir: )
  #   def to_export(replace: "", with: "")
  class EnvProxy::Base
    attr_reader :key

    # Do not create directly
    # or it won't be tracked by EnvProxy instead use env proxy
    # class methods:
    #
    # - EnvPxoxy.value("FOO")
    # - EnvPxoxy.path("FOO_PATH")
    #
    def initialize(key)
      @key = key

      @layer_env_hash = {}
    end

    # Returns one of [:prepend, :append, :override, :default]
    private def layer_env_type
      raise "Must subclass"
    end

    # Used by subclasses to determine how `to_export` behaves
    # for instance when prepending a path, we want the export to
    # always end in the key
    private def value_for_export(replace: "", with: "")
      raise "Must subclass"
    end

    # Returns a formatted string used for writing an export
    # to a bash script
    #
    # Example:
    #
    #  LOL_PATH_ENV = EnvProxy.path("LOL_PATH")
    #  LOL_PATH_ENV.prepend(ruby: "/app/lol")
    #  LOL_PATH_ENV.prepend(gems: "/app/rofl")
    #
    #  puts LOL_PATH.to_export # => 'export LOL_PATH="/app/rofl:/app/lol:$LOL_PATH"
    #  puts LOL_PATH.to_export(replace: "/app", with: "$HOME") # => 'export LOL_PATH="$HOME/rofl:$HOME/lol:$LOL_PATH"
    #
    def to_export(replace: "", with: "")
      value = value_for_export(replace: replace, with: with)
      %Q{export #{key}="#{value}"}
    end

    # Writes the contents of the env var to the given layer
    #
    # Note: That both build and launch layers are written
    #       for every env var, but whether they're used or
    #       not is dependent on how they're configured via
    #       `EnvProxy.register_layer`
    #
    # Example:
    #
    #   LOL_PATH_ENV = EnvProxy.path("LOL_PATH")
    #   LOL_PATH_ENV.prepend(ruby: "/app/lol")
    #   LOL_PATH_ENV.prepend(gems: "/app/rofl")
    #
    #   layers_dir = Pathname.new(Dir.mktmpdir)
    #   LOL_PATH.write_exports(
    #     layers_dir: layers_dir
    #   )
    #
    #   layers_dir.join("ruby/env.launch/LOL_PATH") # => "/app/lol"
    #   layers_dir.join("gems/env.launch/LOL_PATH") # => "/app/rofl"
    #
    def write_layer(layers_dir: )
      @layer_env_hash.each do |name, v|
        layer = Pathname.new(layers_dir).join(name.to_s)
        launch_dir = layer.join("env.launch").tap(&:mkpath)

        build_dir = layer.join("env.build").tap(&:mkpath)
        value = Array(v).join(":")
        build_dir.join(self.layer_key).open("w+") do |f|
          f.write(value)
        end

        launch_dir.join(self.layer_key).open("w+") do |f|
          f.write(value)
        end
      end
    end

    # Writes the contents of the env var to the given profile.d file
    # and export file.
    #
    # profile.d contents are stripped so that any value that starts with
    # the app_dir path is turned into $HOME since the build and
    # runtime directory structure are different
    #
    # Example:
    #
    #  LOL_PATH_ENV = EnvProxy.path("LOL_PATH")
    #  LOL_PATH_ENV.prepend(ruby: "/app/lol")
    #  LOL_PATH_ENV.prepend(gems: "/app/rofl")
    #
    #  profile_d = Tempfile.new
    #  export = Tempfile.new
    #  LOL_PATH.write_exports(
    #    profile_d_path: profile_d.path
    #    export_path: export.path,
    #    app_dir: "/app"
    #  )
    #
    #  puts File.read(profile_d) # => 'export LOL_PATH="$HOME/rofl:$HOME/lol:$LOL_PATH"
    #  puts File.read(export) # => 'export LOL_PATH="/app/rofl:/app/lol:$LOL_PATH"'
    #
    def write_exports(profile_d_path: , export_path: , app_dir: )
      profile_d_path = Pathname.new(profile_d_path)
      export_path = Pathname.new(export_path)

      profile_d_path.open("a") do |f|
        f.write(to_export(replace: app_dir, with: "$HOME"))
      end

      export_path.open("a") do |f|
        f.write(to_export)
      end
    end

    # Generates the appropriate layer file name
    # for the given layer type
    #
    # v = EnvProxy.value("FOO")
    # v.layer_env_type # => :override
    # v.layer_key # => "FOO.override"
    private def layer_key
      case layer_env_type
      when :append, :override, :default
        [key, layer_env_type].join(".")
      when :prepend
        key
      else
        raise "No such layer env type #{layer_env_type}"
      end
    end
  end
end
