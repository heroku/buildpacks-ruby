# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- The `docker run` command no longer requires an entrypoint when using default processes provided by `heroku/ruby` directly (and not the `heroku/procfile` buildpack) ([#404](https://github.com/heroku/buildpacks-ruby/pull/404))

## [6.0.0] - 2025-03-12

### Changed

- Gem install behavior and configuration ([#402](https://github.com/heroku/buildpacks-ruby/pull/402))
  - Gem install path is now configured with `GEM_HOME` and `GEM_PATH` instead of `BUNDLE_PATH`.
  - Cleaning gems is now accomplished via running `bundle clean --force`. Previously it was accomplished by setting `BUNDLE_CLEAN=1`.
  - The `BUNDLE_DEPLOYMENT=1` environment variable is changed to `BUNDLE_FROZEN=1`.
  - The `BUNDLE_BIN` environment variable is no longer set.

## [5.1.0] - 2025-02-28

### Changed

- Enabled `libcnb`'s `trace` feature. ([#398](https://github.com/heroku/buildpacks-ruby/pull/398))

## [5.0.1] - 2025-01-13

### Fixed

- Executables from the applications `bin` directory will be placed on the path before dependencies installed via bundler ([#383](https://github.com/heroku/buildpacks-ruby/pull/383))
- Binaries from user installed gems will be placed on the path before binaries that ship with Ruby ([#383](https://github.com/heroku/buildpacks-ruby/pull/383))

## [5.0.0] - 2024-12-17

### Changed

- Default Ruby version is now 3.2.6 ([#374](https://github.com/heroku/buildpacks-ruby/pull/374))
- Default Bundler version is now 2.5.6 ([#374](https://github.com/heroku/buildpacks-ruby/pull/374))

## [4.0.2] - 2024-12-16

### Fixed

- Ruby pre-release verssions like `3.4.0.rc1` now work as expected. ([#372](https://github.com/heroku/buildpacks-ruby/pull/372))
- Layer metadata deserialization to Rust structs is now using `#[serde(deny_unknown_fields)]` this prevents the accidental scenario where metadata containing a superset of fields could accidentally be deserialized to the wrong struct. It's unlikely this is currently happening with the current buildpack, but it's a possibly-observable difference so it's being listed ([#371](https://github.com/heroku/buildpacks-ruby/pull/371))

## [4.0.1] - 2024-12-11

### Fixed

- A bug introduced in 4.0.0 would result in incorrectly skipping running `bundle install` when the `Gemfile` or `Gemfile.lock` or environment variables had changed. The bug is now fixed. ([#364](https://github.com/heroku/buildpacks-ruby/pull/364))

## [4.0.0] - 2024-11-27

### Changed

- Default process types defined by the Ruby buildpack now use IPv6 host `::` which is equivalent of IPv4 host `0.0.0.0`. This will only affect applications that do not define a `web` process type via the `Procfile` and [Procfile Cloud Native Buildpack](https://github.com/heroku/buildpacks-procfile). Those applications must make sure to update their configuration to bind to an IPv6 host. ([#354](https://github.com/heroku/buildpacks-ruby/pull/354))

### Added

- The buildpack now warns the user when environmental variables used in running the default process are not defined. ([#307](https://github.com/heroku/buildpacks-ruby/pull/307))

## [3.0.0] - 2024-05-17

### Changed

- The buildpack now implements Buildpack API 0.10 instead of 0.9, and so requires `lifecycle` 0.17.x or newer. ([#283](https://github.com/heroku/buildpacks-ruby/pull/283))

### Added

- Added support for Ubuntu 24.04 (and thus Heroku-24 / `heroku/builder:24`). ([#284](https://github.com/heroku/buildpacks-ruby/pull/284))

## [2.1.3] - 2024-03-18

### Changed

- The `fun_run` commons library was moved to it's own crate ([#232](https://github.com/heroku/buildpacks-ruby/pull/232))

### Added

- Raise a helpful error when a file cannot be accessed at the time of buildpack detection ([#243](https://github.com/heroku/buildpacks-ruby/pull/243))

## [2.1.2] - 2023-10-31

### Fixed

- Update build logging style (https://github.com/heroku/buildpacks-ruby/pull/198)

## [2.1.1] - 2023-10-24

### Fixed

- Updated buildpack display name, description and keywords. ([#223](https://github.com/heroku/buildpack-ruby/pull/223))

## [2.1.0] - 2023-09-26

### Added

- Introduce heroku build metrics support (https://github.com/heroku/buildpacks-ruby/pull/172)
- Changelog moved to be per-crate rather than for the whole project (https://github.com/heroku/buildpacks-ruby/pull/154)

## [2.0.1] - 2023-07-25

- Commons: Introduce `build_output` module (https://github.com/heroku/buildpacks-ruby/pull/155)
- Commons: Remove `gem_list`, `rake_status`, `rake_task_detect` modules (https://github.com/heroku/buildpacks-ruby/pull/155)
- Commons: `EnvCommand` removed, replaced with `fun_run` (https://github.com/heroku/buildpacks-ruby/pull/139)

## [2.0.0] - 2023-01-31

- Initial version of Ruby buildpack in Rust (https://github.com/heroku/buildpacks-ruby/pull/93)
- Version 2.0.0 for the first release is not a typo. There was an issue in pack where a builder with the same name and version number would reuse artifacts left on image from [prior runs which caused issues](https://github.com/buildpacks/pack/issues/1322#issuecomment-1038241038). There were prior releases of `heroku/ruby` CNB from different sources that triggered this problem. To ensure no one would encounter that issue we developed and released using a version we know has not been used before. Version 2.0 was the first major version without a prior release of `heroku/ruby` CNB from any source.

[unreleased]: https://github.com/heroku/buildpacks-ruby/compare/v6.0.0...HEAD
[6.0.0]: https://github.com/heroku/buildpacks-ruby/compare/v5.1.0...v6.0.0
[5.1.0]: https://github.com/heroku/buildpacks-ruby/compare/v5.0.1...v5.1.0
[5.0.1]: https://github.com/heroku/buildpacks-ruby/compare/v5.0.0...v5.0.1
[5.0.0]: https://github.com/heroku/buildpacks-ruby/compare/v4.0.2...v5.0.0
[4.0.2]: https://github.com/heroku/buildpacks-ruby/compare/v4.0.1...v4.0.2
[4.0.1]: https://github.com/heroku/buildpacks-ruby/compare/v4.0.0...v4.0.1
[4.0.0]: https://github.com/heroku/buildpacks-ruby/compare/v3.0.0...v4.0.0
[3.0.0]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.3...v3.0.0
[2.1.3]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.2...v2.1.3
[2.1.2]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.1...v2.1.2
[2.1.1]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.0...v2.1.1
[2.1.0]: https://github.com/heroku/buildpacks-ruby/compare/v2.0.1...v2.1.0
[2.0.1]: https://github.com/heroku/buildpacks-ruby/releases/tag/v2.0.1
