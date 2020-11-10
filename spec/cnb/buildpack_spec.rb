# frozen_string_literal: true

require_relative '../spec_helper'

class CnbRun
  attr_accessor :image_name, :output, :repo_path, :buildpack_path, :builder

  def initialize(repo_path, builder: "heroku/buildpacks:18", buildpack_paths: , config: {})
    @repo_path = repo_path
    @image_name = "minimal-heroku-buildpack-ruby-tests:#{SecureRandom.hex}"
    @builder = builder
    @buildpack_paths = Array.new(buildpack_paths)
    @build_output = ""
    @threads = []
    @config = config
  end

  def build!
    command = String.new("pack build #{image_name} --path #{repo_path} --builder heroku/buildpacks:18")
    @config.each do |(k,v)|
      command << %Q{ --env "#{k}=#{v}"}
    end
    @buildpack_paths.each do |path|
      command << " --buildpack #{path}"
    end

    puts command

    @output = run_local!(command)
  end

  def call
    build!

    yield self
  ensure
    teardown
  end

  def teardown
    @threads.map(&:join)

  ensure
    return unless image_name

    repo_name, tag_name = image_name.split(":")

    docker_list = `docker images --no-trunc | grep #{repo_name} | grep #{tag_name}`.strip
    run_local!("docker rmi #{image_name} --force") if !docker_list.empty?
    @image_name = nil
  end

  def run(cmd)
    command = %Q{docker run #{image_name} #{cmd.to_s.shellescape} 2>&1}
    `#{command}`.strip
  end

  def run!(cmd)
    out = run(cmd)
    raise "Command #{cmd.inspect} failed. Output: #{out}" unless $?.success?
    out
  end

  private def run_local!(cmd)
    out = `#{cmd} 2>&1`
    raise "Command #{cmd.inspect} failed. Output: #{out}" unless $?.success?
    out
  end

  def run_multi!(cmd)
    @threads << Thread.new do
      out = run!(cmd)
      status = $?
      yield out, status
    end
  end
end

module HerokuBuildpackRuby
  RSpec.describe "Cloud Native Buildpack" do
    it "locally runs default_ruby app" do
      CnbRun.new(hatchet_path("ruby_apps/default_ruby"), buildpack_paths: [buildpack_path]).call do |app|
        expect(app.output).to include("Installing rake")

        app.run_multi!("ruby -v") do |out|
          expect(out).to match(RubyDetectVersion::DEFAULT)
        end

        app.run_multi!("bundle list") do |out|
          expect(out).to match("rack")
        end

        app.run_multi!("gem list") do |out|
          expect(out).to match("rack")
        end

        app.run_multi!(%Q{ruby -e "require 'rack'; puts 'done'"}) do |out|
          expect(out).to match("done")
        end

        app.build!

        expect(app.output).to include("Using rake")
      end
    end

    it "installs node and yarn and calls assets:precompile" do
      CnbRun.new(hatchet_path("ruby_apps/minimal_webpacker"), buildpack_paths: ["heroku/nodejs", buildpack_path]).call do |app|
        # This output comes from the heroku/nodejs buildpack
        expect(app.output).to include("Installing rake")
        expect(app.output).to include("Installing yarn")

        # This output comes from the contents of the Rakefile
        # https://github.com/sharpstone/minimal_webpacker/blob/master/Rakefile
        expect(app.output).to include("THE TASK ASSETS:PRECOMPILE WAS CALLED")
        expect(app.output).to include("THE TASK ASSETS:CLEAN WAS CALLED")

        app.run_multi!("which node") do |out, status|
          expect(out.strip).to_not be_empty
          expect(status.success?).to be_truthy
        end

        app.run_multi!("which yarn") do |out, status|
          expect(out.strip).to_not be_empty
          expect(status.success?).to be_truthy
        end
      end
    end

    it "Respects user config vars" do
      CnbRun.new(
        hatchet_path("ruby_apps/default_ruby"),
        buildpack_paths: [buildpack_path],
        config: {"BUNDLE_WITHOUT": "periwinkle"}
      ).call do |app|
        expect(app.output).to include(%Q{BUNDLE_WITHOUT="periwinkle"})
      end
    end
  end
end
