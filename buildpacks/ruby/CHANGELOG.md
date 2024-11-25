# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[unreleased]: https://github.com/heroku/buildpacks-ruby/compare/v3.0.0...HEAD
[3.0.0]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.3...v3.0.0
[2.1.3]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.2...v2.1.3
[2.1.2]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.1...v2.1.2
[2.1.1]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.0...v2.1.1
[2.1.0]: https://github.com/heroku/buildpacks-ruby/compare/v2.0.1...v2.1.0
[2.0.1]: https://github.com/heroku/buildpacks-ruby/releases/tag/v2.0.1
