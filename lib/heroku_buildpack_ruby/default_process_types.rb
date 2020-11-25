# frozen_string_literal: true

module HerokuBuildpackRuby
  class DefaultProcessTypes
    private; attr_reader :deps, :rack, :railties, :thin; public

    def initialize(deps)
      @thin = deps.version("thin")
      @rack = deps.version("rack")
      @railties = deps.version("railties")
      @process_hash = { console: "bundle exec irb" }

      case
      when railties
        set_rails_types
      when rack
        set_rack_types
      end
    end

    def to_h
      @process_hash
    end

    private def set_rails_types
      case
      when railties >= Gem::Version.new("4.0")
        set_rails_4_types
      when railties >= Gem::Version.new("3.0")
        set_rails_3_types
      else
        raise "Unsupported version of rails: #{railties}"
      end
    end

    private def set_rails_4_types
      @process_hash[:web]     = "bin/rails server -p ${PORT:-5000} -e $RAILS_ENV"
      @process_hash[:console] = "bin/rails console"
    end

    private def set_rails_3_types
      @process_hash[:console] = "bundle exec rails console"
      if thin
        @process_hash[:web] = "bundle exec thin start -R config.ru -e $RAILS_ENV -p ${PORT:-5000}"
      else
        @process_hash[:web] = "bundle exec rails server -p ${PORT:-5000}"
      end
    end

    private def set_rack_types
      if thin
        @process_hash[:web] = "bundle exec thin start -R config.ru -e $RACK_ENV -p ${PORT:-5000}"
      else
        @process_hash[:web] = "bundle exec rackup config.ru -p ${PORT:-5000}"
      end
    end
  end
end
