
## Develoment

### Application Contract Detect

- [TODO] Node version
  - Given a `package.json` file in the root of the application the `heroku/nodejs` buildpack will be required. [See README for behavior](https://github.com/heroku/buildpacks-nodejs/)
- [TODO] Java version
  - Given a `Gemfile.lock` file that specifies jruby the `heroku/jvm` buildpack will be required. [See README for behavior](https://github.com/heroku/buildpacks-jvm/)

### Application Contract Compile

- Environment variable defaults - We will set a default for the following environment variables:
  - [TODO] RACK_ENV
  - [TODO] RAILS_ENV
  - [TODO] JRUBY_OPTS
  - [TODO] DISABLE_SPRING
  - [TODO] SECRET_KEY_BASE
  - [TODO] MALLOC_ARENA_MAX
  - [TODO] RAILS_LOG_TO_STDOUT
  - [TODO] RAILS_SERVE_STATIC_FILES

- Environment variables modified - In addition to the default list this is a list of environment variables that the buildpack modifies:
  - `BUNDLE_BIN=<bundle-path-dir>/bin` - Install executables for all gems into specified path.
  - `BUNDLE_CLEAN=1` - After successful `bundle install` bundler will automatically run `bundle clean`.
  - `BUNDLE_DEPLOYMENT=1` - Requires the `Gemfile.lock` to be in sync with the current `Gemfile`.
  - `BUNDLE_GEMFILE=<app-dir>/Gemfile` - Tells bundler where to find the `Gemfile`.
  - `BUNDLE_GLOBAL_PATH_APPENDS_RUBY_SCOPE=1` - Append the Ruby engine and ABI version to path. This makes the path's less "surprising".
  - `BUNDLE_PATH=<bundle-path-dir>` - Directs bundler to install gems to this path
  - `BUNDLE_WITHOUT=development:test:$BUNDLE_WITHOUT` - Do not install `development` or `test` groups via bundle isntall. Additional groups can be specified via user config.
  - `GEM_PATH=<bundle-path-dir>` - Tells Ruby where gems are located
  - `NOKOGIRI_USE_SYSTEM_LIBRARIES=1` - Tells `nokogiri` to use the system packages, mostly `openssl`, which Heroku maintains and patches as part of its [stack](https://devcenter.heroku.com/articles/stack-packages). This setting means when a patched version is rolled out on Heroku your application will pick up the new version with no update required to libraries.
  - `PATH` - Various executables are installed and the `PATH` env var will be modified so they can be executed at the system level. This is mostly done via interfaces provided by `libcnb` and CNB layers rather than directly.
  - [TODO] list is incomplete
- Ruby version
  - Given a `Gemfile.lock` with an explicit Ruby version we will install that Ruby version.
  - Given a `Gemfile.lock` without an explicit Ruby version we will install a default Ruby version.
- Bundler version
  - Given a `Gemfile.lock` with an explicit Bundler version we will install that bundler version.
  - Given a `Gemfile.lock` without an explicit Bundler version we will install a default Ruby version.
- Ruby Dependencies
  - We will install gem dependencies using `bundle install`
  - [TODO] We will run `bundle clean` after a successful `bundle install`
  - We will cache the contents of your gem dependencies.
      - [TODO] We will invalidate the dependency cache if your Ruby version changes.
      - [TODO] We will invalidate the dependency cache if your stack changes.
      - [TODO] We may invalidate the dependency cache if there was a bug in a prior buildpack version that needs to be fixed.
- Gem specific behavior - We will parse your `Gemfile.lock` to determine what dependencies your app need for use in specializing your install behavior (i.e. Rails 5 versus Rails 4). The inclusion of these gems may trigger different behavior:
  - `sprockets`
  - `railties`
  - [TODO] List is incomplete
- [TODO] Rake execution - We will determine what rake tasks are runnable via the output of `rake -P` against your application.
  - [TODO] We may abort the build if the `rake -p` task fails.
    - [TODO] If your application has the `sprockets` gem, [then what?]
  - [TODO] Applications expecting Rake task execution must have `rake` in the Gemfile.lock and a `Rakefile` variant checked into the root of their application.
  - [TODO] We will run `rake assets:precompile` on your app if it exists on your application.
    - [TODO] We will skip this task if a manifest file exists in the `public/assets` folder that indicates precompiled assets are checked into git.
    - [TODO] We will abort your build if the `rake assets:precompile` task fails
    - [TODO] We will run `rake assets:clean` on your app.
      - [TODO] We will cache the contents of `public/assets` if `assets:clean` exists on your application.
      - [TODO] We will limit or prune the size of this asset cache.
      - [TODO] We will cache asset "fragments" directories if the `sprockets` gem is on the system.

- Process types
  - Given an application with the `rack` gem and a `config.ru` file we will run `rackup` while specifying `-p $PORT` and `-h 0.0.0.0` by default as the `web` process. Use the `Procfile` to override.

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
