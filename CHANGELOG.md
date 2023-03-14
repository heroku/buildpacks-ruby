# Changelog

## [Unreleased]

## [2.0.0] 2023/31/01

- Initial version of Ruby buildpack in Rust (https://github.com/heroku/buildpacks-ruby/pull/93)
- Version 2.0.0 for the first release is not a typo. There was an issue in pack where a builder with the same name and version number would reuse artifacts left on image from [prior runs which caused issues](https://github.com/buildpacks/pack/issues/1322#issuecomment-1038241038). There were prior releases of `heroku/ruby` CNB from different sources that triggered this problem. To ensure no one would encounter that issue we developed and released using a version we know has not been used before. Version 2.0 was the first major version without a prior release of `heroku/ruby` CNB from any source.
