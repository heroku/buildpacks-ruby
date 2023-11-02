# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The `fun_run` commons library was moved to it's own crate ()

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

[unreleased]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.2...HEAD
[2.1.2]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.1...v2.1.2
[2.1.1]: https://github.com/heroku/buildpacks-ruby/compare/v2.1.0...v2.1.1
[2.1.0]: https://github.com/heroku/buildpacks-ruby/compare/v2.0.1...v2.1.0
[2.0.1]: https://github.com/heroku/buildpacks-ruby/releases/tag/v2.0.1
