# frozen_string_literal: true

require "bundler/setup"

require 'rspec/retry'

ENV["HATCHET_BUILDPACK_BASE"] = "https://github.com/heroku/heroku-buildpack-ruby-experimental.git"

require 'hatchet'
require 'pathname'
require 'tempfile'
require 'stringio'
require 'securerandom'
require 'timeout'
require 'stackprof'

require "dead_end"

require "heroku_buildpack_ruby"
require "cutlass"

def hatchet_path(path = "")
  Pathname(__FILE__).join("../../repos").expand_path.join(path)
end

Cutlass.config do |config|
  # Image pinned to fix CI until https://github.com/heroku/buildpacks-ruby/pull/79 lands. 
  config.default_builder = "heroku/buildpacks:18@sha256:7590c0bc92e574253e44ef3848579869cfeb610b80de093463ef4a7d8de3ae03"

  # Where do your test fixtures live?
  config.default_repo_dirs = [hatchet_path("ruby_apps")]

  # Where does your buildpack live?
  # Can be a directory or a Cutlass:LocalBuildpack instance
  config.default_buildpack_paths = [Pathname(__dir__).join("..")]
end

RSpec.configure do |config|
  # Enable flags like --only-failures and --next-failure
  config.example_status_persistence_file_path = ".rspec_status"
  config.display_try_failure_messages = true
  config.verbose_retry       = true # show retry status in spec process
  config.default_retry_count = 2 if ENV['IS_RUNNING_ON_CI'] # retry all tests that fail again

  config.expect_with :rspec do |c|
    c.syntax = :expect
  end

  ## ENV var set and persist
  config.before(:suite) do
    LOAD_PATH_DUP = $LOAD_PATH.dup

    Cutlass::CleanTestEnv.record
  end

  ## ENV var check
  config.after(:suite) do
    if LOAD_PATH_DUP != $LOAD_PATH
      raise <<~EOM
        LOAD_PATH is mutated
      EOM
    end

    Cutlass::CleanTestEnv.check
  end
end

def run!(cmd)
  out = `#{cmd}`
  raise "Error running #{cmd}, output: #{out}" unless $?.success?
  out
end

def spec_dir
  Pathname(__dir__)
end

def root_dir
  Pathname(__dir__).join("..")
end

def which_ruby
  @which_ruby ||= `which ruby`.strip
end

def which_bundle
  @which_bundle_dir ||= Pathname(`which bundle`.strip)
end

def buildpack_path
  File.expand_path(File.join("../.."), __FILE__)
end


def bash_functions_file
  root_dir.join("bin", "support", "bash_functions.sh")
end

def isolate_in_fork
  Tempfile.create("stdout") do |tmp_file|
    pid = fork do
      $stdout.reopen(tmp_file, "a")
      $stderr.reopen(tmp_file, "a")
      $stdout.sync = true
      $stderr.sync = true
      yield
      Kernel.exit!(0) # needed for https://github.com/seattlerb/minitest/pull/683
    end
    Process.waitpid(pid)

    if $?.success?
      print File.read(tmp_file)
    else
      raise File.read(tmp_file)
    end
  end
end

class String
  def strip_control_codes
    self.gsub(/\e\[[^\x40-\x7E]*[\x40-\x7E]/, "")
  end
end

class My
  class Pathname
    def self.mktmpdir
      Dir.mktmpdir do |dir|
        yield Pathname(dir)
      end
    end
  end
end
