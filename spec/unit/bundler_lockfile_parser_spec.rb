# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "lockfile parser" do
    it "can read dependencies" do
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

        # We need bundler internal locations
        #
        # We could use the already installed bundler on disk,
        # but `which bundler` points to the binstub and not to
        # the install location which may be different, it's easier
        # to just download a copy every time
        bundler_install_dir = dir.join("bundler").tap(&:mkpath)
        BundlerDownload.new(
          version: "2.1.4",
          install_dir: bundler_install_dir
        ).call

        # We need to be able to make sure our code works
        # when no bundler version is loaded, to simulate this
        # we write a script to disk then execute it with a raw
        # `ruby` command.
        script = dir.join("script.rb")
        script.write <<~EOM
          $LOAD_PATH << "#{root_dir.join('lib')}"

          raise "This constant should not be loaded yet Bundler::LockfileParser" if defined?(Bundler::LockfileParser)

          require 'heroku_buildpack_ruby'

          dependencies = HerokuBuildpackRuby::BundlerLockfileParser.new(
            gemfile_lock_path: "#{lockfile}",
            bundler_install_dir: "#{bundler_install_dir}",
          ).call

          puts "Has minitest: \#{dependencies.has_gem?("minitest")}"
          puts "Minitest Version: \#{dependencies.version("minitest")}"
          puts "Windows: \#{!!dependencies.windows?}"
        EOM

        Bundler.with_original_env do
          out = Bash.new("ruby #{script}").run!

          expect(out).to include("Has minitest: true")
          expect(out).to include("Minitest Version: 5.14.0")
          expect(out).to include("Windows: false")
        end
      end
    end
  end
end
