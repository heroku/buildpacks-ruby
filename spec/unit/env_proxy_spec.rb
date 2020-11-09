# frozen_string_literal: true

require_relative "../spec_helper.rb"

RSpec.describe "env proxy" do
  before(:all) do
    HerokuBuildpackRuby::EnvProxy.register_layer(:foo, build: true, cache: true,  launch: true)
    HerokuBuildpackRuby::EnvProxy.register_layer(:bar, build: true, cache: true,  launch: true)
  end

  after(:all) do
    HerokuBuildpackRuby::EnvProxy.delete_layer(:foo)
    HerokuBuildpackRuby::EnvProxy.delete_layer(:bar)
  end

  def unique_env_key
    while key = SecureRandom.hex and ENV.key?(key)
    end
    key
  end

  it "does not let special characters in env vars affect exporting" do
    env_var = HerokuBuildpackRuby::EnvProxy.path(unique_env_key)
    ENV[env_var.key] = "a\nb"
    env_var.prepend(foo: "c")

    expect(env_var.value).to eq("c:a\nb")
    expect(env_var.to_export).to eq(%Q{export #{env_var.key}="c:$#{env_var.key}"})
  ensure
    HerokuBuildpackRuby::EnvProxy.delete(env_var)
  end

  it "lol example" do
    env_var = HerokuBuildpackRuby::EnvProxy.path(unique_env_key)
    env_var.prepend(foo: ["/app/lol", "haha"])
    env_var.prepend(bar: "/app/rofl")

    profile_d = Tempfile.new
    export = Tempfile.new
    env_var.write_exports(
      profile_d_path: profile_d.path,
      export_path: export.path,
      app_dir: "/app"
    )

    expect(
      File.read(profile_d).strip
    ).to eq(%Q{export #{env_var.key}="$HOME/rofl:$HOME/lol:haha:$#{env_var.key}"})
  ensure
    HerokuBuildpackRuby::EnvProxy.delete(env_var)
  end

  it "exports to a file" do
    env_var_1 = HerokuBuildpackRuby::EnvProxy.value(unique_env_key)
    env_var_1.set(
      bar: "/app/cinco"
    )

    env_var_2 = HerokuBuildpackRuby::EnvProxy.path(unique_env_key)
    env_var_2.prepend(
      bar: "/app/river"
    )
    profile_d = Tempfile.new
    export = Tempfile.new
    HerokuBuildpackRuby::EnvProxy.export(
      profile_d_path: profile_d.path,
      export_path: export.path,
      app_dir: "/app"
    )

    expect(File.read(export.path)).to include(%Q{export #{env_var_1.key}="/app/cinco"\n})
    expect(File.read(export.path)).to include(%Q{export #{env_var_2.key}="/app/river:$#{env_var_2.key}"\n})

    expect(File.read(profile_d.path)).to include(%Q{export #{env_var_1.key}="$HOME/cinco"\n})
    expect(File.read(profile_d.path)).to include(%Q{export #{env_var_2.key}="$HOME/river:$#{env_var_2.key}"\n})

    Dir.mktmpdir do |dir|
      layers_dir = Pathname(dir)
      HerokuBuildpackRuby::EnvProxy.write_layers(
        layers_dir: layers_dir,
      )

      toml_hash = HerokuBuildpackRuby::TOML.load(layers_dir.join("foo.toml").read)
      expect(toml_hash).to eq({build: true, cache: true, launch: true})

      expect(layers_dir.join("bar/env.launch").entries.map(&:to_s)).to include("#{env_var_1.key}.override")

      expect(layers_dir.join("bar/env.launch/#{env_var_1.key}.override").read).to eq("/app/cinco")
      expect(layers_dir.join("bar/env.launch/#{env_var_2.key}").read).to eq("/app/river")

      expect(layers_dir.join("bar/env.build/#{env_var_1.key}.override").read).to eq("/app/cinco")
      expect(layers_dir.join("bar/env.build/#{env_var_2.key}").read).to eq("/app/river")
    end
  ensure
    HerokuBuildpackRuby::EnvProxy.delete(env_var_1) if env_var_1
    HerokuBuildpackRuby::EnvProxy.delete(env_var_2) if env_var_2
  end

  it "prevents setting values to two different env vars" do
    env_var = HerokuBuildpackRuby::EnvProxy.value(unique_env_key)
    expect {
      env_var.set(
        foo: "/hi/there/hi",
        bar: "i am different"
      )
    }.to raise_error(/cannot set the same ENV var/)
  end

  it "default will use the user env value if one is present" do
    key = unique_env_key
    Dir.mktmpdir do |dir|
      dir = Pathname(dir)
      dir.join(key).write("lol")

      user_env = HerokuBuildpackRuby::UserEnvFromDir.new.parse(dir)
      env_var = HerokuBuildpackRuby::EnvProxy.default(key, user_env: user_env)
      env_var.set_default(
        foo: "/hi/there/hi",
      )

      expect(HerokuBuildpackRuby::EnvProxy).to include(env_var)

      # Modifies ENV
      expect(ENV[env_var.key]).to eq("lol")
      expect(env_var.value).to eq("lol")
      expect(env_var.to_env).to eq(%Q{#{env_var.key}="lol" })
    end
  ensure
    ENV.delete(key) if key
  end

  it "default acts like a default" do
    env_var = HerokuBuildpackRuby::EnvProxy.default(unique_env_key)
    env_var.set_default(
      foo: "/hi/there/hi",
    )

    expect(HerokuBuildpackRuby::EnvProxy).to include(env_var)

    # Modifies ENV
    expect(ENV[env_var.key]).to eq("/hi/there/hi")
    expect(env_var.value).to eq("/hi/there/hi")
    expect(env_var.to_env).to eq(%Q{#{env_var.key}="/hi/there/hi" })

    # Exports for legacy/v2 interface
    expect(env_var.to_export).to eq(%Q{export #{env_var.key}="${#{env_var.key}:-/hi/there/hi}"})

    expect(env_var.to_export).to eq(%Q{export #{env_var.key}="${#{env_var.key}:-/hi/there/hi}"})
    expect(env_var.to_export(replace: "/hi", with: "$HOME")).to eq(%Q{export #{env_var.key}="${#{env_var.key}:-$HOME/there/hi}"})

    # Can write to layers for CNB interface
    Dir.mktmpdir do |dir|
      layers_dir = Pathname(dir)

      env_var.write_layer(layers_dir: layers_dir)


      expect(layers_dir.entries.map(&:to_s)).to include("foo")
      expect(layers_dir.join("foo").entries.map(&:to_s)).to include("env.launch")
      expect(layers_dir.join("foo").entries.map(&:to_s)).to include("env.build")

      expect(layers_dir.join("foo/env.launch/#{env_var.key}.default").read).to eq("/hi/there/hi")
      expect(layers_dir.join("foo/env.build/#{env_var.key}.default").read).to eq("/hi/there/hi")
    end
  ensure
    HerokuBuildpackRuby::EnvProxy.delete(env_var) if env_var
  end

  it "value acts like an value-ish" do
    env_var = HerokuBuildpackRuby::EnvProxy.value(unique_env_key)
    env_var.set(
      foo: "/hi/there/hi",
    )

    expect(HerokuBuildpackRuby::EnvProxy).to include(env_var)
    expect(env_var.value).to eq("/hi/there/hi")
    expect(env_var.to_env).to eq(%Q{#{env_var.key}="/hi/there/hi" })

    # Modifies ENV
    expect(ENV[env_var.key]).to eq("/hi/there/hi")

    # Exports for legacy/v2 interface
    expect(env_var.to_export).to eq(%Q{export #{env_var.key}="/hi/there/hi"})

    expect(env_var.to_export).to eq(%Q{export #{env_var.key}="/hi/there/hi"})
    expect(env_var.to_export(replace: "/hi", with: "$HOME")).to eq(%Q{export #{env_var.key}="$HOME/there/hi"})

    # Can write to layers for CNB interface
    Dir.mktmpdir do |dir|
      layers_dir = Pathname(dir)

      env_var.write_layer(layers_dir: layers_dir)


      expect(layers_dir.entries.map(&:to_s)).to include("foo")
      expect(layers_dir.join("foo").entries.map(&:to_s)).to include("env.launch")
      expect(layers_dir.join("foo").entries.map(&:to_s)).to include("env.build")

      expect(layers_dir.join("foo/env.launch/#{env_var.key}.override").read).to eq("/hi/there/hi")
      expect(layers_dir.join("foo/env.build/#{env_var.key}.override").read).to eq("/hi/there/hi")
    end
  ensure
    HerokuBuildpackRuby::EnvProxy.delete(env_var) if env_var
  end

  it "path acts like an array-ish" do
    env_var = HerokuBuildpackRuby::EnvProxy.path(unique_env_key)
    env_var.prepend(
      foo: ["/hi/you", "there"],
      bar: ["how", "are_you"]
    )
    expect(HerokuBuildpackRuby::EnvProxy).to include(env_var)

    # Modifies ENV
    expect(ENV[env_var.key]).to eq("/hi/you:there:how:are_you")
    expect(env_var.value).to eq("/hi/you:there:how:are_you")
    expect(env_var.to_env).to eq(%Q{#{env_var.key}="/hi/you:there:how:are_you" })

    # Exports for legacy/v2 interface
    expect(env_var.to_export).to include(%Q{export #{env_var.key}="how:are_you:/hi/you:there:$#{env_var.key}"})

    expect(env_var.to_export(replace: "/hi", with: "$HOME")).to include(%Q{export #{env_var.key}="how:are_you:$HOME/you:there:$#{env_var.key}"})

    # Can write to layers for CNB interface
    Dir.mktmpdir do |dir|
      layers_dir = Pathname(dir)

      env_var.write_layer(layers_dir: layers_dir)

      expect(layers_dir.entries.map(&:to_s)).to include("foo")
      expect(layers_dir.join("foo").entries.map(&:to_s)).to include("env.launch")
      expect(layers_dir.join("foo").entries.map(&:to_s)).to include("env.build")

      expect(layers_dir.join("foo/env.launch/#{env_var.key}").read).to eq("/hi/you:there")

      env_var.write_layer(layers_dir: layers_dir)
      expect(layers_dir.join("bar").entries.map(&:to_s)).to include("env.launch")
      expect(layers_dir.join("bar").entries.map(&:to_s)).to include("env.build")

      expect(layers_dir.join("bar/env.launch/#{env_var.key}").read).to eq("how:are_you")
    end
  ensure
    HerokuBuildpackRuby::EnvProxy.delete(env_var) if env_var
  end
end
