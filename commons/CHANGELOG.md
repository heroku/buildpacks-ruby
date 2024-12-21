# Changelog for commons features

### Added

- Introduced `layer::diff_migrate` and `DiffMigrateLayer` for public cache use (https://github.com/heroku/buildpacks-ruby/pull/376)

### Changed

- The `layer` module is no longer deprecated, only `layer::ConfigureEnvLayer` and `layer::DefaultEnvLayer` (https://github.com/heroku/buildpacks-ruby/pull/376)

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
