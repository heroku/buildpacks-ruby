require "pathname"

module HerokuBuildpackRuby
  # Downloads bundler
  #
  # Example:
  #
  #   puts `ls /tmp/bundler`.inspect # => ""
  #   bundler_download = BundlerDownload.new(version: "2.1.4", install_dir: "/tmp/bundler")
  #
  #   bundler_download.call
  #   puts `ls /tmp/bundler`.inspect # => "cache\nbin\gems\nspecifications\nbuild_info\nextensions\ndoc"
  class BundlerDownload
    attr_reader :version, :install_dir

    def initialize(version:, install_dir: )
      @install_dir = Pathname.new(install_dir).tap(&:mkpath)
      @version = version
    end

    def call
      return true if install_dir.join("bin").exist?

      # Install directory structure (as of Bundler 2.1.4):
      # - cache
      # - bin
      # - gems
      # - specifications
      # - build_info
      # - extensions
      # - doc
      CurlFetch.new(
        "bundler-#{version}.tgz",
        folder: "bundler",
        install_dir: install_dir
      ).fetch_untar
    end
  end
end
