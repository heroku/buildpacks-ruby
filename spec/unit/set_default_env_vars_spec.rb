# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "SetDefaultEnvVars" do
    it "sets env vars" do
      isolate_in_fork do
        expected = {
          "RACK_ENV" => "production",
          "RAILS_ENV" => "production",
          "JRUBY_OPTS" => "-Xcompile.invokedynamic=false",
          "DISABLE_SPRING" => "1",
          "MALLOC_ARENA_MAX" => "2",
          "RAILS_LOG_TO_STDOUT" => "enabled",
          "RAILS_SERVE_STATIC_FILES" => "enabled",
        }
        expected.each {|key, _| ENV.delete(key) } # Pristine environment

        null = Metadata::Null.new
        SetDefaultEnvVars.new(
          metadata: null,
          environment: "production",
        ).call

        expected.each { |key, value| expect(ENV[key]).to eq(value) }

        expect(ENV["SECRET_KEY_BASE"]).to_not be_empty
        expect(ENV["SECRET_KEY_BASE"].length).to eq(SecureRandom.hex(64).length)
      end
    end

    it "persists secret key base" do
      isolate_in_fork do
        null = Metadata::Null.new
        null.layer(:ruby).set(:secret_key_base_default => "i can neither confirm nor deny")

        SetDefaultEnvVars.new(
          metadata: null,
          environment: "production",
        ).call

        expect(ENV["SECRET_KEY_BASE"]).to eq("i can neither confirm nor deny")
      end
    end
  end
end
