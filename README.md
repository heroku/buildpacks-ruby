
## Develoment

### Application Contract

- Ruby version
  - Given a Gemfile.lock with an explicit Ruby version we will install that Ruby version
  - Given a Gemfile.lock without an explicit Ruby version we will install a default Ruby version
- Bundler version
  - Given a Gemfile.lock with an explicit Bundler version we will install that bundler version
  - Given a Gemfile.lock without an explicit Bundler version we will install a default Ruby version
- Ruby Dependencies
  - We will install gem dependencies using `bundle install`
- Process types
  - Given an application with the `rack` gem and a `config.ru` file we will run `rackup` while specifying `-p $PORT` and `-h 0.0.0.0` by default as the `web` process.

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
