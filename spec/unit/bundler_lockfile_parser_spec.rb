require_relative "../spec_helper.rb"

RSpec.describe "lockfile parser" do
  it "can read dependencies" do
    isolate_in_fork do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)
        lockfile = dir.join("Gemfile.lock")
        lockfile.write <<~EOM
          PATH
            remote: .
            specs:
              mini_histogram (0.3.1)

          GEM
            remote: https://rubygems.org/
            specs:
              benchmark-ips (2.7.2)
              m (1.5.1)
                method_source (>= 0.6.7)
                rake (>= 0.9.2.2)
              method_source (0.9.2)
              minitest (5.14.0)
              rake (12.3.3)

          PLATFORMS
            ruby

          DEPENDENCIES
            benchmark-ips
            m
            mini_histogram!
            minitest (~> 5.0)
            rake (~> 12.0)

          BUNDLED WITH
             2.1.4
        EOM

        bundler_install_dir = dir.join("bundler").tap(&:mkpath)
        HerokuBuildpackRuby::BundlerDownload.new(
          version: "2.1.4",
          install_dir: bundler_install_dir
        ).call


        Bundler.send(:remove_const, :LockfileParser)

        expect(defined?(Bundler::LockfileParser)).to be_falsey

        dependencies = HerokuBuildpackRuby::BundlerLockfileParser.new(
          gemfile_lock_path: lockfile,
          bundler_install_dir: bundler_install_dir,
        ).call

        expect(dependencies.bundler_lib_path.to_s).to eq(bundler_install_dir.join("gems/bundler-2.1.4/lib").to_s)
        expect(dependencies.has_gem?("minitest")).to be_truthy
        expect(dependencies.version("minitest")).to eq(Gem::Version.new("5.14.0"))
        expect(dependencies.windows?).to be_falsey
      end
    end
  end
end
