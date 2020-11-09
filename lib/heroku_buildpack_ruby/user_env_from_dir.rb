require "shellwords"

module HerokuBuildpackRuby
  # This parses and emulates an ENV object for user
  # supplied values
  #
  # Example:
  #
  #   dir.join("HELLO").write("there")
  #
  #   env = UserEnvFromDir.new.parse(dir)
  #   puts env["HELLO"] # => "there
  class UserEnvFromDir
    def initialize(deny_list: [])
      @hash = {}
      @deny_list = deny_list
      @deny_list.concat %W{PATH GEM_PATH GEM_HOME GIT_DIR}
      @deny_list.concat %W{JRUBY_OPTS JAVA_OPTS JAVA_TOOL_OPTIONS}
    end

    def parse(dir)
      dir = Pathname(dir)
      dir.entries.sort.each do |entry|
        path = dir.join(entry)
        next if path.directory?

        key = path.basename.to_s
        @hash[key] = path.read unless @deny_list.include?(key)
      end

      self
    end

    def empty?
      @hash.empty?
    end

    def any?
      !empty?
    end

    def to_shell
      @hash.map {|key, value| %Q{#{key.shellescape}="#{value.shellescape}"} }.join(" ")
    end

    def [](key)
      @hash[key]
    end

    def key?(key)
      @hash.key?(key)
    end

    def clear
      @hash.clear
    end
  end

  UserEnv = UserEnvFromDir.new
end
