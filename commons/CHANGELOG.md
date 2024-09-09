# Changelog for commons features

## 1.1.0

### Changed

- `AppCacheCollection` is deprecated. Use a `Vec<AppCache>` instead.
- `configure_env_layer::ConfigureEnvLayer` and `default_env_layer::DefaultEnvLayer` are deprecated, use the new "struct api" for layers instead.

## 1.0.0

### Changed

- Move `fun_run` commons library to it's own crate (https://github.com/heroku/buildpacks-ruby/pull/232/files)
