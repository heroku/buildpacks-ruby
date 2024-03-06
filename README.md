# Heroku Cloud Native Buildpack: Ruby

[![Cloud Native Buildpacks Registry: heroku/ruby][registry-badge]][registry-url]
[![CI on Github Actions: heroku/ruby][ci-badge]][ci-url]

![Heroku Cloud Native Buildpack: heroku/ruby][cnb-banner]

`heroku/ruby` is the [Heroku Cloud Native Buildpack][heroku-buildpacks]
for Ruby applications. It builds Ruby application source code into application images with
minimal configuration.

> [!IMPORTANT]
> This is a [Cloud Native Buildpack][cnb], and is a component of the [Heroku Cloud Native Buildpacks][heroku-buildpacks] project, which is in beta. If you are instead looking for the Heroku Classic Buildpack for Ruby (for use on the Heroku platform), you may find it [here][classic-buildpack].

## Usage

> [!NOTE]
> Before getting started, ensure you have the `pack` CLI installed. Installation instructions are available [here][pack-install].

To build a Ruby application codebase into a production image:

```bash
$ cd ~/workdir/sample-ruby-app
$ pack build sample-app --builder heroku/builder:22
```

Then run the image:
```bash
docker run --rm -it -e "PORT=8080" -p 8080:8080 sample-app
```

## Application contract

The sections below describe the expected behavior of the buildpack. The codebase must be updated if a difference exists between this contract and actual behavior. Either the code needs to change to suit the contract, or the contract needs to be updated. If you see a difference, please open an issue with a [minimal application that reproduces the problem](https://www.codetriage.com/example_app).

If you need application-specific support, you can ask on an official Heroku support channel or Stack Overflow.

### Application Contract: Detect

The detect phase determines whether or not this buildpack can execute. It can also be used to request additional functionality via requiring behavior from other buildpacks.

- Node version
  - Given a `package.json` file in the root of the application the `heroku/nodejs-engine` buildpack will be required. [See README for behavior](https://github.com/heroku/buildpacks-nodejs/tree/main/buildpacks/nodejs-engine)
- Yarn version
  - Given a `yarn.lock` file in the root of the application the `heroku/nodejs-yarn` buildpack will be required. [See README for behavior](https://github.com/heroku/buildpacks-nodejs/tree/main/buildpacks/nodejs-yarn)
- Java version
  - Given a `Gemfile.lock` file that specifies jruby the `heroku/jvm` buildpack will be required. [See README for behavior](https://github.com/heroku/buildpacks-jvm/)
- Ruby version
  - Given a `Gemfile.lock` this buildpack will execute the Ruby build contract below.

### Application Contract: Build

Once an application has passed the detect phase, the build phase will execute to prepare the application to run.

- Ruby version:
  - Given a `Gemfile.lock` with an explicit Ruby version, we will install that Ruby version.
  - Given a `Gemfile.lock` without an explicit Ruby version, we will install a default Ruby version.
    - When the default value changes, applications without an explicit Ruby version will receive the updated version on their next deployment.
  - We will reinstall Ruby if your stack (operating system) changes.
- Bundler version:
  - Given a `Gemfile.lock` with an explicit Bundler version we will install that bundler version.
  - Given a `Gemfile.lock` without an explicit Bundler version we will install a default Ruby version.
- Ruby Dependencies:
  - We MAY install gem dependencies using `bundle install`
    - We will always run `bundle install` for the first build.
    - We will sometimes run this command again if we detect one of the following has changed:
      - `Gemfile`
      - `Gemfile.lock`
      - User configurable environment variables.
    -To always run `bundle install` even if there are changes if the environment variable `HEROKU_SKIP_BUNDLE_DIGEST=1` is found.
  - We will always run `bundle clean` after a successful `bundle install` via setting `BUNDLE_CLEAN=1` environment variable.
  - We will always cache the contents of your gem dependencies.
      - We will always invalidate the dependency cache if your stack (operating system) changes.
      - We will always invalidate the dependency cache if your Ruby version changes.
      - We may invalidate the dependency cache if there was a bug in a prior buildpack version that needs to be fixed.
- Gem specific behavior - We will parse your `Gemfile.lock` to determine what dependencies your app need for use in specializing your install behavior (i.e. Rails 5 versus Rails 4). The inclusion of these gems may trigger different behavior:
  - `railties`
- Applications without `rake` in the `Gemfile.lock` or a `Rakefile` variant MAY skip rake task detection.
- Rake execution - We will determine what rake tasks are runnable via the output of `rake -P` against your application.
  - We will always abort the build if the `rake -p` task fails.
  - We will always run `rake assets:precompile` on your app if that task exists for your application.
    - We will always skip this `assets:precompile` task if a manifest file exists in the `public/assets` folder that indicates precompiled assets are checked into git.
      - `.sprockets-manifest-*.json`
      - `manifest-*.json`
    - We will abort your build if the `rake assets:precompile` task fails.
    - We will run `rake assets:clean` on your app.
      - We will cache the contents of `public/assets` if `assets:clean` exists on your application.
      - We will cache asset "fragments" directories if the `assets:clean` exists on the system.
      - We will limit or prune the size of the asset cache in `tmp/cache/assets` to 100 MiB.
        - We will delete the least recently used (LRU) files first. Detected via file mtime.
- Process types:
  - Given an application with the `railties` gem:
    - We will default the web process to `bin/rails server` while specifying `-p $PORT` and `-e $RAILS_ENV"`. Use the `Procfile` to override this default.
  - If `railties` gem is not found but `rack` gem is present and a `config.ru` file exists on root:
    - We will default the web process to `rackup` while specifying `-p $PORT` and `-h 0.0.0.0`. Use the `Procfile` to override this default. .
- Environment variable defaults - We will set a default for the following environment variables:
  - `JRUBY_OPTS="-Xcompile.invokedynamic=false"` - Invoke dynamic is a feature of the JVM intended to enhance support for dynamicaly typed languages (such as Ruby). This caused issues with Physion Passenger 4.0.16 and was disabled [details](https://github.com/heroku/heroku-buildpack-ruby/issues/145). You can override this value.
  - `RACK_ENV=${RACK_ENV:-"production"}` - An environment variable that may affect the behavior of Rack based webservers and webapps. You can override this value.
  - `RAILS_ENV=${RAILS_ENV:-"production"}` - A value used by all Rails apps. By default, Rails ships with three environments: `development`, `test,` and `production`. We recommend all apps being deployed to use `production` and recommend against using a custom env such as `staging` [details](https://devcenter.heroku.com/articles/deploying-to-a-custom-rails-environment). You can override this value.
  - `SECRET_KEY_BASE=${SECRET_KEY_BASE:-<generate a secret key>}` - In Rails 4.1+ apps a value is needed to generate cryptographic tokens used for a variety of things. Notably this value is used in generating user sessions so modifying it between builds will have the effect of logging out all users. This buildpack provides a default generated value. You can override this value.
  - `BUNDLE_WITHOUT=development:test` - Tells bundler to not install `development` or `test` groups during `bundle install`. You can override this value.
- Environment variables modified - In addition to the default list this is a list of environment variables that the buildpack modifies:
  - `BUNDLE_BIN=<bundle-path-dir>/bin` - Install executables for all gems into specified path.
  - `BUNDLE_CLEAN=1` - After successful `bundle install` bundler will automatically run `bundle clean` to remove all stale gems from previous builds that are no longer specified in the `Gemfile.lock`.
  - `BUNDLE_DEPLOYMENT=1` - Requires `Gemfile.lock` to be in sync with the current `Gemfile`.
  - `BUNDLE_GEMFILE=<app-dir>/Gemfile` - Tells bundler where to find the `Gemfile`.
  - `BUNDLE_PATH=<bundle-path-dir>` - Directs bundler to install gems to this path
  - `DISABLE_SPRING="1"` - Spring is a library that attempts to cache application state by forking and manipulating processes with the goal of decreasing development boot time. Disabling it in production removes significant problems [details](https://devcenter.heroku.com/changelog-items/1826).
  - `GEM_PATH=<bundle-path-dir>` - Tells Ruby where gems are located.
  - `MALLOC_ARENA_MAX=2` - Controls glibc memory allocation behavior with the goal of decreasing overall memory allocated by Ruby [details](https://devcenter.heroku.com/changelog-items/1683).
  - `PATH` - Various executables are installed and the `PATH` env var will be modified so they can be executed at the system level. This is mostly done via interfaces provided by `libcnb` and CNB layers rather than directly.
  - `RAILS_LOG_TO_STDOUT="enabled"` - Sets the default logging target to STDOUT for Rails 5+ apps. [details](https://blog.heroku.com/container_ready_rails_5)
  - `RAILS_SERVE_STATIC_FILES="enabled"` - Enables the `ActionDispatch::Static` middleware for Rails 5+ apps so that static files such as those in `public/assets` are served by the Ruby webserver such as Puma [details](https://blog.heroku.com/container_ready_rails_5).

### Next-gen application contract

These are tracked things the buildpack will eventually do in the application contract but are not currently implemented.

- [TODO] Warn on invalid Ruby binstub (https://github.com/heroku/heroku-buildpack-ruby/blob/main/lib/language_pack/helpers/binstub_wrapper.rb, and https://github.com/heroku/heroku-buildpack-ruby/blob/main/lib/language_pack/helpers/binstub_check.rb)
- [TODO] Warn on default ruby version (https://github.com/heroku/heroku-buildpack-ruby/blob/26065614274ed1138620806c9a8d705e39b4412c/lib/language_pack/ruby.rb#L532)
- [TODO] Warn on Ruby version not in Gemfile.lock, but present in `bundle platform --ruby`
- [TODO] Warn on outdated ruby version (https://github.com/heroku/heroku-buildpack-ruby/blob/main/lib/language_pack/helpers/outdated_ruby_version.rb)
- [TODO] Run `rails runner` to collect configuration, abort a build if it fails (https://github.com/heroku/heroku-buildpack-ruby/blob/main/lib/language_pack/helpers/rails_runner.rb)
- [TODO] Warn when `RAILS_ENV=staging` (https://github.com/heroku/heroku-buildpack-ruby/blob/2b567c597d5cb110774eb21b9616b311d8e4ee9d/lib/language_pack/rails2.rb#L65-L71)
- [TODO] Warn if we cannot run a Web process
  - [TODO] Warn no Procfile (https://github.com/heroku/heroku-buildpack-ruby/blob/26065614274ed1138620806c9a8d705e39b4412c/lib/language_pack/base.rb#L156-L158)
  - [TODO] Warn Railties, but no `bin/rails` binstub (new)
  - [TODO] Warn no Procfile, no Railties, and missing: `rack` gem or `config.ru` file. (new)
- [TODO-never-fail-rake] Make a buildpack that moves the contents of the Rakefile into another file like `heroku_buildpack_wrapped_rakefile.rb` and then replaces Rake with:

```
begin
  require_relative "heroku_buildpack_wrapped_rakefile.rb"
rescue => e
  STDERR.puts <<~EOM
    Error caught and ignored while loading Rakefile

    To fail the build instead, remove the buildpack TODO

    Message:
    #{e.message}
  EOM
end
```

### Known differences against `heroku/heroku-buildpack-ruby`

This buildpack does not port all behaviors of the [original buildpack for Ruby ](https://github.com/heroku/heroku-buildpack-ruby), which is also known as "classic". This section outlines the known behaviors where the two buildpacks diverge.

- Rails 5+ support only. The classic buildpack supports Rails 2+. There are significant maintenance gains for buildpack authors [starting in Rails 5](https://blog.heroku.com/container_ready_rails_5) which was released in 2016. In an effort to reduce overall internal complexity this buildpack does not explicitly support Rails before version 5.
- Rails support is now based on the above application contract. Previously there was no official policy for dropping support for older Rails versions. Support was based on whether or not an older version could run on a currently supported Ruby version. With [Rails LTS](https://railslts.com/) this can mean a very old version of Rails. Now we will actively support Rails versions currently [under the Rails core maintenance policy](https://guides.rubyonrails.org/maintenance_policy.html) provided those versions can run on a supported Ruby version. As Rails versions go out of official maintenance compatibility features may be removed with no supported replacement.
- Ruby versions come from the `Gemfile.lock` only. Before bundler started recording the Ruby version in the `Gemfile.lock`, Heroku would pull the Ruby version via running `bundle platform --ruby` to pull any ruby declaration such as `ruby "3.1.2"` from the `Gemfile`. This creates a bootstrapping problem, because you need a version of Ruby to run `bundle platform` to find the version of Ruby the application needs. Since the Ruby version is now recorded in the `Gemfile.lock`, this extra bootstrapping problem is less needed and applications should rely on their version being in their `Gemfile.lock` instead. To update the Ruby version in `Gemfile.lock` run `bundle update --ruby`.
- Default Ruby versions are no longer sticky. Before bundler started recording the Ruby version in the `Gemfile.lock` it was common for developers to forget to declare their Ruby version in their project. When that happens they would receive the default Ruby version from the classic buildpack. To guard against instability the classic buildpack would record that version and use it again on all future deploys where no explicit version was specified. Now it's less likely for an app to deploy without a ruby version in their `Gemfile.lock`, and we do not want to encourage relying on a default for stability (as this otherwise breaks dev-prod parity).
- Failure to detect rake tasks will fail a deployment. On all builds `rake -p` is called to find a list of all rake tasks. If this detection task fails then the previous behavior was to fail the build only if the `sprockets` gem was present. The reason was to allow API-only apps that don't need to generate assets to have a `Rakefile` that cannot be run in production (the most common reason is they're requiring a library not in their production gemfile group). Now all failures when loading a `Rakefile` will fail the build. If you want the old behavior you can [TODO](TODO-never-fail-rake). The reason for this change is that it's more common that applications will want their builds to fail even if they're not using `sprockets`. It's also just not a good idea to not have your `Rakefile` not runable in production, we shouldn't encourage that pattern.
- Caching of `public/assets` is gated on the presence of `rake assets:clean`. Previously this behavior was gated on the existence of a certain version of the Rails framework.
- Caching of `tmp/cache/assets` (fragments) is gated on the presence of `rake assets:clean`. Previously this behavior was gated on the existence of a certain version of the Rails framework.


## Contributing

Issues and pull requests are welcome. See our [contributing guidelines](./CONTRIBUTING.md) if you would like to help.


[ci-badge]: https://github.com/heroku/buildpacks-ruby/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/heroku/buildpacks-ruby/actions/workflows/ci.yml
[cnb]: https://buildpacks.io
[cnb-banner]: https://cloud.githubusercontent.com/assets/51578/13712725/3c6b3368-e793-11e5-83c1-728440111358.png
[classic-buildpack]: https://github.com/heroku/heroku-buildpack-ruby
[heroku-buildpacks]: https://github.com/heroku/buildpacks
[pack-install]: https://buildpacks.io/docs/for-platform-operators/how-to/integrate-ci/pack/
[registry-badge]: https://img.shields.io/badge/dynamic/json?url=https://registry.buildpacks.io/api/v1/buildpacks/heroku/ruby&label=version&query=$.latest.version&color=DF0A6B&logo=data:image/png;base64,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&labelColor=white
[registry-url]: https://registry.buildpacks.io/buildpacks/heroku/ruby

