# Changelog for commons features

## 2024-11-11

## Changed

- Deprecate `layer` including `layer::ConfigureEnvLayer` and `layer::DefaultEnvLayer` (https://github.com/heroku/buildpacks-ruby/pull/345)
- Remove `AppCacheCollection` (https://github.com/heroku/buildpacks-ruby/pull/345)
- Deprecate `output` module in favor of the `bullet_stream` crate (https://github.com/heroku/buildpacks-ruby/pull/345)

## 2024-08-16

### Fixed

- `AppCache` will now preserve mtime of files when copying them to/from the cache (https://github.com/heroku/buildpacks-ruby/pull/336)

## 2024-08-15

### Changed

- Deprecate `AppCacheCollection` (https://github.com/heroku/buildpacks-ruby/pull/334)

## 1.0.0

### Changed

- Move `fun_run` commons library to it's own crate (https://github.com/heroku/buildpacks-ruby/pull/232/files)
