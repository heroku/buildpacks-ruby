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
  #
  # Metadata: [:bundler][:version]
  class BundlerDownload
    private; attr_reader :metadata, :user_comms, :install_dir; public;
    public; attr_reader :version

    def initialize(version:, install_dir: , user_comms: UserComms::Null.new, metadata: Metadata::Null.new)
      @install_dir = Pathname.new(install_dir).tap(&:mkpath)
      @user_comms = user_comms
      @metadata = metadata.layer(:bundler)
      @version = version
    end

    def call
      return download if !install_dir.join("bin").exist?

      if metadata.get(:version) == version
        user_comms.topic("Using bundler #{metadata.get(:version)}")
      else
        download
      end
    end

    private def download
      user_comms.topic("Installing bundler #{version}")
      metadata.set(version: version)

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
