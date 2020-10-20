module HerokuBuildpackRuby
  # Removes the BUNDLED WITH statement from a lockfile
  #
  # This is used as an "escape valve" from this "feature" in bundler.
  # Some versions of bundler check the BUNDLED WITH version in the Gemfile.lock
  # and then perform extra behavior based on the contents such as raising an exception
  # or attempting to load a different bundler version.
  #
  # This is implemented via a tight coupling between both rubygems and bundler
  # Most of this behavior was not triggered until Bundler version 2.x shipped
  # at which time years-old codepaths in rubygems were executed. In some of these
  # older rubygems versions bugs were present.
  #
  # Since Ruby ships with a Rubygems version and since Heroku does not separately
  # manage rubygems (the version of Rubygems that's present when Ruby core cuts a release
  # of Ruby is the version you'll get) means that random bugs will appear in relatively random
  # versions of Ruby on the platform.
  #
  # In an attempt to mitigate this behavior, we're choosing to delete this line from the Gemfile.lock.
  #
  # Example:
  #
  #   lockfile = Pathname.new("Gemfile.lock")
  #   puts lockfile.include?("BUNDLED WITH") # => true
  #   BundlerStripBundledWith.new(lockfile).call
  #   puts lockfile.include?("BUNDLED WITH") # => false
  class BundlerStripBundledWith
    BUNDLED_WITH_REGEX = /^BUNDLED WITH$(\r?\n)   (?<major_version>\d+)\.\d+\.\d+/m

    private; attr_reader :user_comms, :lockfile_path; public

    def initialize(lockfile_path: , user_comms: UserComms::Null.new)
      @user_comms = user_comms
      @lockfile_path = Pathname.new(lockfile_path)
    end

    def call
      user_comms.topic("Removing BUNDLED WITH from Gemfile.lock")
      contents = lockfile_path.read.sub(BUNDLED_WITH_REGEX, '')
      lockfile_path.write(contents)
      contents
    end
  end
end
