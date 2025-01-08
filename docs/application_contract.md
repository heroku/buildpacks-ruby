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
  - We will reinstall Ruby if your distribution name or version (operating system) changes.
  - We will reinstall Ruby if your CPU architecture (i.e. amd64) changes.
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
      - We will always invalidate the dependency cache if your distribution name or version (operating system) changes.
      - We will always invalidate the dependency cache if your CPU architecture (i.e. amd64) changes.
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
    - We will default the web process to `bin/rails server` while specifying `--port $PORT`, `--environment $RAILS_ENV"` and an IPv6 host with `--binding "::"` (equivalent of IPv4 host `0.0.0.0`). Use the `Procfile` to override this default.
  - If `railties` gem is not found but `rack` gem is present and a `config.ru` file exists on root:
    - We will default the web process to `rackup` while specifying `--port $PORT` and IPv6 host with `--host "::"` (equivalent of IPv4 host `0.0.0.0`). Use the `Procfile` to override this default. .
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
    - Binaries from gems will take precedence over binaries that ship with Ruby (for example `rake` installed from `bundle install` should be loaded before `rake` that come with the compiled Ruby binary).
  - `RAILS_LOG_TO_STDOUT="enabled"` - Sets the default logging target to STDOUT for Rails 5+ apps. [details](https://blog.heroku.com/container_ready_rails_5)
  - `RAILS_SERVE_STATIC_FILES="enabled"` - Enables the `ActionDispatch::Static` middleware for Rails 5+ apps so that static files such as those in `public/assets` are served by the Ruby webserver such as Puma [details](https://blog.heroku.com/container_ready_rails_5).
