require_relative "../spec_helper.rb"

RSpec.describe "bash_functions.sh that need docker" do
  it "compiles node apps" do
    Hatchet::Runner.new("minimal_webpacker").in_directory do
      contents = <<~EOM
        #! /usr/bin/env bash
        set -eu
        export STACK="heroku-18"

        #{bash_functions_file.read}


        build_dir="/tmp/build_dir"
        cache_dir="/tmp/cache_dir"
        env_dir="/tmp/env_dir"

        mkdir -p "$cache_dir"
        mkdir -p "$env_dir"
        mkdir -p "$build_dir"
        cp -r /myapp/* "$build_dir"

        compile_buildpack_v2 "$build_dir" "$cache_dir" "$env_dir" "https://buildpack-registry.s3.amazonaws.com/buildpacks/heroku/nodejs.tgz" "heroku/nodejs"

        echo "which node $(which node)"
        echo "which yarn $(which yarn)"
      EOM

      script = Pathname.new(".").join("script.sh")
      script.write(contents)
      FileUtils.chmod("+x", script)

      output = `docker run -v "$PWD:/myapp" -it --rm heroku/heroku:18-build /myapp/script.sh 2>&1`
      expect(output).to include("Build succeeded")
      expect(output).to include("installing node")
      expect(output).to include("installing yarn")

      expect(output).to include("which node /tmp/build_dir/.heroku/node/bin/node")
      expect(output).to include("which yarn /tmp/build_dir/.heroku/yarn/bin/yarn")

      expect($?.success?).to be_truthy
    end
  end
end
