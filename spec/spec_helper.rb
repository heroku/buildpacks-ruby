require "bundler/setup"

require 'rspec/retry'

ENV["HATCHET_BUILDPACK_BASE"] = "https://github.com/schneems/minimal-ruby.git"

require 'hatchet'
require 'pathname'
require 'tempfile'
require 'stringio'
require 'securerandom'
require 'timeout'

require "heroku_buildpack_ruby"

RSpec.configure do |config|
  # Enable flags like --only-failures and --next-failure
  config.example_status_persistence_file_path = ".rspec_status"
  config.verbose_retry       = true # show retry status in spec process
  config.default_retry_count = 2 if ENV['IS_RUNNING_ON_CI'] # retry all tests that fail again

  config.expect_with :rspec do |c|
    c.syntax = :expect
  end
end

def run!(cmd)
  out = `#{cmd}`
  raise "Error running #{cmd}, output: #{out}" unless $?.success?
  out
end

def spec_dir
  Pathname.new(__dir__)
end

def root_dir
  Pathname.new(__dir__).join("..")
end

def which_ruby
  @which_ruby ||= `which ruby`.strip
end

def which_bundle
  @which_bundle_dir ||= Pathname.new(`which bundle`.strip)
end

def buildpack_path
  File.expand_path(File.join("../.."), __FILE__)
end

def hatchet_path(path = "")
  Pathname.new(__FILE__).join("../../repos").expand_path.join(path)
end
