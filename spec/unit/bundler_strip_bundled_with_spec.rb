# frozen_string_literal: true

require_relative "../spec_helper.rb"

RSpec.describe "BundlerStripBundledWith" do
  it "removes bundled with contents from disk" do
    Tempfile.create("Gemfile.lock") do |lockfile|
      lockfile = Pathname(lockfile)
      lockfile.write <<~EOM
        before
        BUNDLED WITH
           2.1.4
        after
      EOM

      expect(lockfile.read).to include("BUNDLED WITH")
      HerokuBuildpackRuby::BundlerStripBundledWith.new(lockfile_path: lockfile).call
      expect(lockfile.read).to_not include("BUNDLED WITH")
      expect(lockfile.read.strip).to eq("before\n\nafter")
    end
  end
end
