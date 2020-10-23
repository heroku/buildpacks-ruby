module HerokuBuildpackRuby
  # Detect the bundler version of an app's Gemfile.lock
  #
  # And convert it into a supported bundler version.
  #
  # Example:
  #
  #   puts `cat Gemfile.lock | grep "BUNDLED WITH" -a1`.inspect
  #   # => "BUNDLED WITH\n   2.0.4"
  #
  #   bundler_detect = BundlerDetectVersion.new(lockfile_path: "Gemfile.lock")
  #   bundler_detect.call
  #
  #   puts bundler_detect.version == BundlerDetectVersion::BUNDLER_VERSIONS["2"]
  #   # => true
  class BundlerDetectVersion
    BUNDLER_VERSIONS = {}
    BUNDLER_VERSIONS["1"] = "1.17.3"
    BUNDLER_VERSIONS["2"] = "2.1.4"
    BUNDLER_VERSIONS[nil] = BUNDLER_VERSIONS["1"]
    BUNDLED_WITH_REGEX = /^BUNDLED WITH$(\r?\n)   (?<major_version>\d+)\.\d+\.\d+/m

    private; attr_reader :lockfile_path; public;

    def initialize(lockfile_path: )
      @lockfile_path = Pathname(lockfile_path)
      @version = nil
    end

    def call
      @version ||= begin
        match = lockfile_path.read(mode: "rt").match(BUNDLED_WITH_REGEX)
        major_version = match ? match[:major_version] : nil
        BUNDLER_VERSIONS[major_version]
      end
      self
    end

    def version
      @version || raise("Must execute `call` to set bundler version")
    end
  end
end
