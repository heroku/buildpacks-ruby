module HerokuBuildpackRuby
  # Used for downloading files and unpacking them if needed
  #
  # It automatically sets the stack directory
  #
  #   puts ENV["STACK"] # => "heroku-20"
  #
  #   curl_fetch = CurlFetch.new("ruby-3.0.0.tgz", install_dir: "/tmp/here")
  #   curl_fetch.url # => https://s3-external-1.amazonaws.com/heroku-buildpack-ruby/heroku-18/ruby-3.0.0.tgz
  #
  # To skip this behvior you can specify your own `folder:` or use `folder: nil` to disable.
  #
  # Example:
  #
  #   curl_fetch = CurlFetch.new("bundler/bundler-2.1.4.tgz", install_dir: "/tmp/here", folder: nil)
  #   curl_fetch.url # => https://s3-external-1.amazonaws.com/heroku-buildpack-ruby/bundler-2.1.4.tgz
  #
  #   curl_fetch = CurlFetch.new("bundler/bundler-2.1.4.tgz", install_dir: "/tmp/here", folder: "bundler")
  #   curl_fetch.url # => https://s3-external-1.amazonaws.com/heroku-buildpack-ruby/bundler/bundler-2.1.4.tgz
  class CurlFetch
    VENDOR_HOST_URL = ENV['BUILDPACK_VENDOR_URL'] || "https://s3-external-1.amazonaws.com/heroku-buildpack-ruby"

    attr_reader :url

    def initialize(path, install_dir: , host_url: VENDOR_HOST_URL, folder: ENV["STACK"])
      @url = Pathname(host_url).join(folder.to_s).join(path)
      @install_dir = install_dir

      # TODO support CURL_TIMEOUT and CURL_CONNECT_TIMEOUT
      @curl_command_prefix = "set -o pipefail; curl -L --fail --retry 5 --retry-delay 1 --connect-timeout 10 --max-time 120 ".freeze
    end

    def fetch_untar(files_to_extract: nil)
      Dir.chdir(@install_dir) do
        Bash.new(
          "#{@curl_command_prefix} #{@url} -s -o - | tar zxf - #{files_to_extract}",
          max_attempts: 3
        ).run!
      end
    end
  end
end
