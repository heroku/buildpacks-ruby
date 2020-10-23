module HerokuBuildpackRuby
  # Parses a Gemfile.lock so the contents can be inspected
  #
  # Example:
  #
  #   dependencies = BundlerLockfileParser.new(gemfile_lock_path: "./Gemfile.lock", bundler_install_dir: bundler_install_dir).call
  #   dependencies.has_gem?("rails") # => true
  #   dependencies.version("rails") # => Gem::Version.new("6.0.0")
  class BundlerLockfileParser
    private; attr_reader :parser, :gemfile_lock_path, :bundler_install_dir; public

    def initialize(gemfile_lock_path: , bundler_install_dir: )
      @bundler_install_dir = Pathname(bundler_install_dir)
      @gemfile_lock_path = Pathname(gemfile_lock_path)
      @platforms = nil
      @gem_specs = nil
      @parser = nil
    end

    # Loads the lockfile parser from Bundler source into memory so we can use it
    # to parse Gemfile.lock returns self
    def call
      require_relative bundler_lib_path.join("bundler/lockfile_parser.rb")
      self
    end

    def has_gem?(name)
      gems.key?(name)
    end

    def version(name)
      gems[name]&.version
    end

    def windows?
      platforms.detect do |platform|
        /mingw|mswin/.match?(platform.os) if platform.is_a?(Gem::Platform)
      end
    end

    def bundler_lib_path
      bundler_install_dir
        .join("gems")
        .glob("bundler-*").first
        .join("lib")
    end

    private def gems
      @gem_specs ||= lockfile_parser.specs.each_with_object({}) {|spec, hash| hash[spec.name] = spec }
    end

    private def platforms
      @platforms ||= lockfile_parser.platforms
    end

    private def lockfile_parser
      @lockfile_parser ||= begin
        raise "Bundler lockfile parser is not loaded you must :call first" unless defined?(Bundler::LockfileParser)
        Bundler::LockfileParser.new(gemfile_lock_path.read)
      end
    end
  end
end
