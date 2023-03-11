# Changelog

- Added: Set `HEROKU_USE_SYSTEM_RUBY` to disable installation of a Ruby version. This tells the Ruby buildpack that you plan to manage your own Ruby version manually. This setting is experimental and voids support guarantees. (https://github.com/heroku/buildpacks-ruby/pull/130)
- Added: Allow "*" stacks, though not all stacks are officially supported. (https://github.com/heroku/buildpacks-ruby/pull/130)
- Changed: Remove `gemfile_lock.rs` from commons. (https://github.com/heroku/buildpacks-ruby/pull/130)

## Unreleased

- Initial version of Ruby buildpack in Rust (https://github.com/heroku/buildpacks-ruby/pull/93)
