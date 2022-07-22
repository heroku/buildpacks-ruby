## Develoment

### Application Contract Detect

- [TODO] Node version
  - Given a `package.json` file in the root of the application the `heroku/nodejs` buildpack will be required. [See README for behavior](https://github.com/heroku/buildpacks-nodejs/)
- [TODO] Java version
  - Given a `Gemfile.lock` file that specifies jruby the `heroku/jvm` buildpack will be required. [See README for behavior](https://github.com/heroku/buildpacks-jvm/)

### Application Contract Compile

- Ruby version
  - Given a `Gemfile.lock` with an explicit Ruby version we will install that Ruby version.
  - Given a `Gemfile.lock` without an explicit Ruby version we will install a default Ruby version.
  - We will reinstall Ruby if your stack changes.
- Bundler version
  - Given a `Gemfile.lock` with an explicit Bundler version we will install that bundler version.
  - Given a `Gemfile.lock` without an explicit Bundler version we will install a default Ruby version.
- Ruby Dependencies
  - We will install gem dependencies using `bundle install`
  - We will run `bundle clean` after a successful `bundle install` via setting `BUNDLE_CLEAN=1` environment variable.
  - We will cache the contents of your gem dependencies.
      - We will invalidate the dependency cache if your stack changes.
      - We will invalidate the dependency cache if your Ruby version changes.
      - We may invalidate the dependency cache if there was a bug in a prior buildpack version that needs to be fixed.
- Gem specific behavior - We will parse your `Gemfile.lock` to determine what dependencies your app need for use in specializing your install behavior (i.e. Rails 5 versus Rails 4). The inclusion of these gems may trigger different behavior:
  - `sprockets`
  - `railties`
  - [TODO] List is incomplete
- Rake execution - We will determine what rake tasks are runnable via the output of `rake -P` against your application.
  - We may abort the build if the `rake -p` task fails.
    - If your application has the `sprockets` gem and `rake -p` failed the build will abort.
  - [TODO] Applications expecting Rake task execution must have `rake` in the Gemfile.lock and a `Rakefile` variant checked into the root of their application. (Reference: https://github.com/heroku/buildpacks-ruby/blob/d526a49f81becaf571329a1adf5fff0668a6b99a/lib/heroku_buildpack_ruby/rake_detect.rb#L41-L58)
  - We will run `rake assets:precompile` on your app if that task exists on your application.
    - [TODO] We will skip this `assets:precompile` task if a manifest file exists in the `public/assets` folder that indicates precompiled assets are checked into git. (Reference: https://github.com/heroku/buildpacks-ruby/blob/d526a49f81becaf571329a1adf5fff0668a6b99a/lib/heroku_buildpack_ruby/assets_precompile.rb#L29)
    - We will abort your build if the `rake assets:precompile` task fails
    - [TODO] We will run `rake assets:clean` on your app. (Reference: https://github.com/heroku/buildpacks-ruby/blob/d526a49f81becaf571329a1adf5fff0668a6b99a/lib/heroku_buildpack_ruby/assets_precompile.rb#L67-L73, reference: https://github.com/heroku/heroku-buildpack-ruby/blob/951fc728979695990c32df2d4d60ae2d6f6f61c2/lib/language_pack/rails4.rb#L83-L94)
      - [TODO] We will cache the contents of `public/assets` if `assets:clean` exists on your application. (pending https://github.com/buildpacks/spec/blob/main/buildpack.md#slice-layers support in libcnbrs)
      - [TODO] We will cache asset "fragments" directories if the `sprockets` gem is on the system. (pending https://github.com/buildpacks/spec/blob/main/buildpack.md#slice-layers support in libcnbrs)
        - [TODO] We will limit or prune the size of the asset cache in `tmp/<TBD>`. (Need to write this logic in rust, StaleFileSweep, reference: https://github.com/heroku/heroku-buildpack-ruby/blob/main/lib/language_pack/helpers/stale_file_cleaner.rb)
- Process types
  - Given an application with the `rack` gem and a `config.ru` file we will run `rackup` while specifying `-p $PORT` and `-h 0.0.0.0` by default as the `web` process. Use the `Procfile` to override.
- Environment variable defaults - We will set a default for the following environment variables:
  - `JRUBY_OPTS="-Xcompile.invokedynamic=false"` - Invoke dynamic is a feature of the JVM intended to enhance support for dynamicaly typed languages (such as Ruby). This caused issues with Physion Passenger 4.0.16 and was disabled [details](https://github.com/heroku/heroku-buildpack-ruby/issues/145).
  - `RACK_ENV=${RACK_ENV:-"production"}` - An environment variable that may affect the behavior of Rack based webservers and webapps.
  - `RAILS_ENV=${RAILS_ENV:-"production"}` - A value used by all Rails apps. By default Rails ships with three environments: `development`, `test,` and `production`. We recommend all apps being deployed to use `production` and recommend against using a custom env such as `staging` [details](https://devcenter.heroku.com/articles/deploying-to-a-custom-rails-environment).
  - `SECRET_KEY_BASE=${SECRET_KEY_BASE:-<generate a secret key>}` - In Rails 4.1+ apps a value is needed to generate crypographic tokens used for a variety of things. Notably this value is used in generating user sessions so modifying it between builds will have the effect of logging out all users. Heroku provides a default generated value.
- Environment variables modified - In addition to the default list this is a list of environment variables that the buildpack modifies:
  - `BUNDLE_BIN=<bundle-path-dir>/bin` - Install executables for all gems into specified path.
  - `BUNDLE_CLEAN=1` - After successful `bundle install` bundler will automatically run `bundle clean`.
  - `BUNDLE_DEPLOYMENT=1` - Requires the `Gemfile.lock` to be in sync with the current `Gemfile`.
  - `BUNDLE_GEMFILE=<app-dir>/Gemfile` - Tells bundler where to find the `Gemfile`.
  - `BUNDLE_PATH=<bundle-path-dir>` - Directs bundler to install gems to this path
  - `BUNDLE_WITHOUT=development:test:$BUNDLE_WITHOUT` - Do not install `development` or `test` groups via bundle isntall. Additional groups can be specified via user config.
  - `DISABLE_SPRING="1"` - Spring is a library that attempts to cache application state by forking and manipulating processes with the goal of decreasing development boot time. Disabling it in production removes significant problems [details](https://devcenter.heroku.com/changelog-items/1826).
  - `GEM_PATH=<bundle-path-dir>` - Tells Ruby where gems are located
  - `MALLOC_ARENA_MAX=2` - Controls glibc memory allocation behavior with the goal of decreasing overall memory allocated by Ruby [details](https://devcenter.heroku.com/changelog-items/1683).
  - `NOKOGIRI_USE_SYSTEM_LIBRARIES=1` - Tells `nokogiri` to use the system packages, mostly `openssl`, which Heroku maintains and patches as part of its [stack](https://devcenter.heroku.com/articles/stack-packages). This setting means when a patched version is rolled out on Heroku your application will pick up the new version with no update required to libraries.
  - `PATH` - Various executables are installed and the `PATH` env var will be modified so they can be executed at the system level. This is mostly done via interfaces provided by `libcnb` and CNB layers rather than directly.
  - `RAILS_LOG_TO_STDOUT="enabled"` - Sets the default logging target to STDOUT for Rails 5+ apps. [details](https://blog.heroku.com/container_ready_rails_5)
  - `RAILS_SERVE_STATIC_FILES="enabled"` - Enables the `ActionDispatch::Static` middleware for Rails 5+ apps so that static files such as those in `public/assets` are served by the Ruby webserver such as Puma [details](https://blog.heroku.com/container_ready_rails_5).

## Next-gen application contract

These are tracked things the buildpack will eventually do in the application contract but are not currently priorities.

- [TODO] Warn on invalid Ruby binstub (https://github.com/heroku/heroku-buildpack-ruby/blob/main/lib/language_pack/helpers/binstub_wrapper.rb, and https://github.com/heroku/heroku-buildpack-ruby/blob/main/lib/language_pack/helpers/binstub_check.rb)
- [TODO] Warn on outdated ruby version (https://github.com/heroku/heroku-buildpack-ruby/blob/main/lib/language_pack/helpers/outdated_ruby_version.rb)
- [TODO] Run `rails runner` to collect configuration, abort a build if it fails (https://github.com/heroku/heroku-buildpack-ruby/blob/main/lib/language_pack/helpers/rails_runner.rb)
- [TODO] Warn when `RAILS_ENV=staging` (https://github.com/heroku/heroku-buildpack-ruby/blob/2b567c597d5cb110774eb21b9616b311d8e4ee9d/lib/language_pack/rails2.rb#L65-L71)

### Build

- Follow setup instructions on https://github.com/Malax/libcnb.rs
- Run:

```
cargo libcnb package
```

- Build the app:

```
pack build my-image --buildpack target/buildpack/debug/heroku_ruby --path tests/fixtures/default_ruby
```

- Validate that it's working:

```
$ docker run -it --rm --entrypoint='/cnb/lifecycle/launcher' my-image 'which ruby'
/layers/heroku_ruby/ruby/bin
$ docker run -it --rm --entrypoint='/cnb/lifecycle/launcher' my-image 'ruby -v'
ruby 2.7.4p191 (2021-07-07 revision a21a3b7d23) [x86_64-linux]
```

Make sure it doesn't say `/usr/bin/ruby` or another system ruby location

As a oneliner:

```
cargo libcnb package && \
docker rmi my-image --force  && \
pack build my-image --buildpack target/buildpack/debug/heroku_ruby --path tests/fixtures/default_ruby && \
docker run -it --rm --entrypoint='/cnb/lifecycle/launcher' my-image 'which bundle'
```

Run the webserver:

```
$ docker run -it --rm --env PORT=9292 -p 9292:9292 my-image --debug
```


Inspect the image:

```
$ pack inspect my-image
```
