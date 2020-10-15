curl_retry_on_18() {
  local ec=18;
  local attempts=0;
  while (( ec == 18 && attempts++ < 3 )); do
    curl "$@" # -C - would return code 33 if unsupported by server
    ec=$?
  done
  return $ec
}

# Fetches a ruby_version key from a given toml file
#
# Example:
#
#  ruby_version_from_toml "/path/to/file.toml"
#  # => "2.6.6"
#
ruby_version_from_toml()
{
  toml_path=$1

  # Pull ruby version out of buildpack.toml to be used with bootstrapping
  regex=".*ruby_version = [\'\"]([0-9]+\.[0-9]+\.[0-9]+)[\'\"].*"
  if [[ $(cat "$toml_path") =~ $regex ]]
    then
      ruby_version="${BASH_REMATCH[1]}"
      echo "$ruby_version"
    else
      echo "Could not detect ruby version to bootstrap"
      exit 1
  fi
}

# Downloads and unpacks a ruby binary to the given directory
#
# Example:
#
#   download_ruby "2.7.1" "/tmp/download_location"
#
download_ruby()
{
  ruby_version=$1
  RUBY_BOOTSTRAP_DIR=$2

  heroku_buildpack_ruby_url="https://s3-external-1.amazonaws.com/heroku-buildpack-ruby/$STACK/ruby-$ruby_version.tgz"

  mkdir -p "$RUBY_BOOTSTRAP_DIR"

  curl_retry_on_18 --fail --silent --location -o "$RUBY_BOOTSTRAP_DIR/ruby.tgz" "$heroku_buildpack_ruby_url" || {
cat<<EOF
  Failed to download a Ruby executable for bootstrapping!

  This is most likely a temporary internal error. If the problem
  persists, make sure that you are not running a custom or forked
  version of the Heroku Ruby buildpack which may need updating.
EOF
  exit 1
}

  tar xzf "$RUBY_BOOTSTRAP_DIR/ruby.tgz" -C "$RUBY_BOOTSTRAP_DIR"
}

# Example:
#
#   BUILDPACK_DIR="/tmp/path/to/buildpack_dir"
#   bootstrap_ruby_from_version "2.6.6"
#
download_ruby_version_to_buildpack_vendor()
{
  local ruby_version=$1
  heroku_buildpack_ruby_dir="$BUILDPACK_DIR/vendor/ruby/$STACK"

  # The -d flag checks to see if a file exists and is a directory.
  # This directory may be non-empty if a previous compile has
  # already placed a Ruby executable here. Also
  # when the buildpack is deployed we vendor a ruby executable
  # at this location so it doesn't have to be downloaded for
  # every app compile
  if [ ! -d "$heroku_buildpack_ruby_dir" ]; then
    download_ruby "$ruby_version" "$heroku_buildpack_ruby_dir"

    # function atexit {
    #   rm -rf $heroku_buildpack_ruby_dir
    # }
    # trap atexit EXIT
  fi
}

bootstrap_ruby_to_buildpack_dir()
{
  ruby_version=$(ruby_version_from_toml "$BUILDPACK_DIR/buildpack.toml")
  download_ruby_version_to_buildpack_vendor "$ruby_version"
}

# Call this instead of `ruby` when you want to use the bootstrapped
# version of ruby
#
# Example:
#
#   buildpack_ruby -v # => 2.6.6
buildpack_ruby_path()
{
  echo $BUILDPACK_DIR/vendor/ruby/$STACK/bin/ruby
}
