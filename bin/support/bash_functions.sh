#!/usr/bin/env bash

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

# Runs another buildpack against the build dir
#
# Example:
#
#   compile_buildpack_v2 "$build_dir" "$cache_dir" "$env_dir" "https://buildpack-registry.s3.amazonaws.com/buildpacks/heroku/nodejs.tgz" "heroku/nodejs"
#
compile_buildpack_v2()
{
  BUILD_DIR=$1
  CACHE_DIR=$2
  ENV_DIR=$3
  BUILDPACK=$4
  NAME=$5

  dir=$(mktemp -t buildpackXXXXX)
  rm -rf $dir

  url=${BUILDPACK%#*}
  branch=${BUILDPACK#*#}

  if [ "$branch" == "$url" ]; then
    branch=""
  fi

  if [ "$url" != "" ]; then
    echo "-----> Downloading Buildpack: ${NAME}"

    if [[ "$url" =~ \.tgz$ ]] || [[ "$url" =~ \.tgz\? ]]; then
      mkdir -p "$dir"
      curl_retry_on_18 -s "$url" | tar xvz -C "$dir" >/dev/null 2>&1
    else
      git clone $url $dir >/dev/null 2>&1
    fi
    cd $dir

    if [ "$branch" != "" ]; then
      git checkout $branch >/dev/null 2>&1
    fi

    # we'll get errors later if these are needed and don't exist
    chmod -f +x $dir/bin/{detect,compile,release} || true

    framework=$($dir/bin/detect $1)

    if [ $? == 0 ]; then
      echo "-----> Detected Framework: $framework"
      $dir/bin/compile $1 $2 $3

      if [ $? != 0 ]; then
        exit 1
      fi

      # check if the buildpack left behind an environment for subsequent ones
      if [ -e $dir/export ]; then
        source $dir/export
      fi

      if [ -x $dir/bin/release ]; then
        $dir/bin/release $1 > $1/last_pack_release.out
      fi
    else
      echo "Couldn't detect any framework for this buildpack. Exiting."
      exit 1
    fi
  fi
}

# A wrapper for `which node` so we can stub it out in tests
which_node()
{
  which node
}

# Returns truthy if the project needs node installed but does not
# have a package.json for example if a Gem in the gemfile depends on node
needs_package_json()
{
  local app_dir=$1
  local truthy=0
  local falsey=1

  # If it already has it, don't over-write
  if [ -f "$app_dir/package.json" ];then
    return $falsey
  fi

  if grep -Fq "execjs" "$app_dir/Gemfile.lock";then
    return $truthy
  else
    return $falsey
  fi
}

detect_needs_node()
{

  local app_dir=$1

  local needs_node=0
  local skip_node_install=1

  if which_node; then
    return $skip_node_install
  fi

  if [ -f "$app_dir/package.json" ];then
    return $needs_node
  else
    return $skip_node_install
  fi
}

# Writes a plan.json that provides and requires ruby as well as asking for node
write_to_build_plan_ruby_node()
{
  local build_plan=$1

cat << EOF > "$build_plan"
[[provides]]
name = "ruby"

[[requires]]
name = "node"

[[requires]]
name = "ruby"
EOF
}

# Writes a plan.json that provides and requires ruby
write_to_build_plan_ruby()
{
  local build_plan=$1

cat << EOF > "$build_plan"
[[provides]]
name = "ruby"

[[requires]]
name = "ruby"
EOF
}


# Writes out the build plan according to contents of the
# app dir.
#
# If the app dir contains files indicating that it needs nodejs installed
# then we output a build plan asking for node otherwise we only ask (and provide)
# ruby.
#
# write_to_build_plan "$PLAN" "$APP_DIR"
write_to_build_plan()
{
  local build_plan=$1
  local build_dir=$2

  if detect_needs_node "$build_dir"; then
    write_to_build_plan_ruby_node "$build_plan"
  else
    write_to_build_plan_ruby "$build_plan"
  fi
}
