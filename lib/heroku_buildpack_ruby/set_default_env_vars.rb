# frozen_string_literal: true

require "securerandom"

module HerokuBuildpackRuby
  # Sets default env vars for ruby apps
  #
  # Example:
  #
  #   SetDefaultEnvVars.new(
  #     metadata: metadata,
  #     environment: "production"
  #   ).call
  #
  #   puts ENV["RAILS_ENV"]
  #   # => "production"
  class SetDefaultEnvVars
    RACK_ENV = EnvProxy.default("RACK_ENV")
    RAILS_ENV = EnvProxy.default("RAILS_ENV")
    JRUBY_OPTS = EnvProxy.default("JRUBY_OPTS")

    DISABLE_SPRING_ENV = EnvProxy.default("DISABLE_SPRING")
    SECRET_KEY_BASE_ENV = EnvProxy.default("SECRET_KEY_BASE")
    MALLOC_ARENA_MAX_ENV = EnvProxy.default("MALLOC_ARENA_MAX")

    RAILS_LOG_TO_STDOUT_ENV = EnvProxy.default("RAILS_LOG_TO_STDOUT")
    RAILS_SERVE_STATIC_FILES_ENV = EnvProxy.default("RAILS_SERVE_STATIC_FILES")

    private; attr_reader :environment, :metadata; public

    def initialize(environment:, metadata: )
      @metadata = metadata.layer(:ruby)
      @environment = environment
    end

    def call
      RACK_ENV.set_default(ruby: environment)
      RAILS_ENV.set_default(ruby: environment)
      JRUBY_OPTS.set_default(ruby: "-Xcompile.invokedynamic=false")

      DISABLE_SPRING_ENV.set_default(ruby: "1")
      SECRET_KEY_BASE_ENV.set_default(ruby: secret_key_base)
      MALLOC_ARENA_MAX_ENV.set_default(ruby: "2")

      RAILS_LOG_TO_STDOUT_ENV.set_default(ruby: "enabled")
      RAILS_SERVE_STATIC_FILES_ENV.set_default(ruby: "enabled")
    end

    private def secret_key_base
      metadata.fetch(:secret_key_base_default) { SecureRandom.hex(64) }
    end
  end
end

