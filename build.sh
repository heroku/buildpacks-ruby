#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd $(dirname "${BASH_SOURCE[0]}") && pwd)

source "$SCRIPT_DIR/bin/support/bash_functions.sh"
BUILDPACK_DIR="$SCRIPT_DIR"

STACK="heroku-18"
bootstrap_ruby_to_buildpack_dir

STACK="heroku-20"
bootstrap_ruby_to_buildpack_dir

targetdir="$SCRIPT_DIR/target"

if [ -d "$targetdir" ]; then
    rm -rf "$targetdir"
fi
mkdir "$targetdir"

# Point target dir at current dir
# Can't use a symlink because 🤷🏻‍♂️
# https://superuser.com/a/180653
TARGETDIR='target';for file in *;do test "$file" != "$TARGETDIR" && cp -r "$file" "$TARGETDIR/";done
