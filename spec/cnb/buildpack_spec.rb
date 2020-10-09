require_relative '../spec_helper'

class CnbRun
  attr_accessor :image_name, :output, :repo_path, :buildpack_path, :builder

  def initialize(repo_path, builder: "heroku/buildpacks:18", buildpack_paths: )
    @repo_path = repo_path
    @image_name = "minimal-heroku-buildpack-ruby-tests:#{SecureRandom.hex}"
    @builder = builder
    @buildpack_paths = Array.new(buildpack_paths)
    @build_output = ""
  end

  def call
    command = String.new("pack build #{image_name} --path #{repo_path} --builder heroku/buildpacks:18")
    @buildpack_paths.each do |path|
      command << " --buildpack #{path}"
    end

    @output = run_local!(command)
    yield self
  ensure
    teardown
  end

  def teardown
    return unless image_name
    repo_name, tag_name = image_name.split(":")

    docker_list = `docker images --no-trunc | grep #{repo_name} | grep #{tag_name}`.strip
    run_local!("docker rmi #{image_name} --force") if !docker_list.empty?
    @image_name = nil
  end

  def run(cmd)
    `docker run #{image_name} '#{cmd}'`.strip
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
end

RSpec.describe "Cloud Native Buildpack" do
  it "locally runs default_ruby app" do
    CnbRun.new(hatchet_path("ruby_apps/default_ruby"), buildpack_paths: [buildpack_path]).call do |app|
      run_out = app.run!("ruby -v")
      expect(run_out).to match(HerokuBuildpackRuby::RubyDetectVersion::DEFAULT)
    end
  end
end
