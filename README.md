
## Develoment

### Build

- Follow setup instructions on https://github.com/Malax/libcnb.rs
- Run:

```
cargo libcnb package
```

- Make a tmp app:

```
mkdir -p /tmp/bogus-ruby-app
cd /tmp/bogus-ruby-app
touch Gemfile
bundle install

echo "puts 'lol'" > app.rb
cd -
```

- Build the app:

```
pack build my-image --buildpack target/buildpack/debug/heroku_ruby --path /tmp/bogus-ruby-app
```

- Validate that it's working:

```
$ docker run --entrypoint='/cnb/lifecycle/launcher' my-image 'which ruby'
/layers/heroku_ruby/ruby/bin
$ docker run --entrypoint='/cnb/lifecycle/launcher' my-image 'ruby -v'
ruby 2.7.4p191 (2021-07-07 revision a21a3b7d23) [x86_64-linux]
```

Make sure it doesn't say `/usr/bin/ruby` or another system ruby location

As a oneliner:

```
cargo libcnb package && pack build my-image --buildpack target/buildpack/debug/heroku_ruby --path /tmp/bogus-ruby-app && docker run --entrypoint='/cnb/lifecycle/launcher' my-image 'which bundle'
```

