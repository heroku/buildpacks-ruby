# Contributing Guide for Heroku Cloud Native Buildpacks

This page lists the operational governance model of this project, as well as the recommendations and requirements for how to best contribute to Heroku Cloud Native Buildpacks. We strive to obey these as best as possible. As always, thanks for contributing.

## Governance Model: Salesforce Sponsored

The intent and goal of open sourcing this project is to increase the contributor and user base. However, only Salesforce employees will be given `admin` rights and will be the final arbitrars of what contributions are accepted or not.

## Getting started

Please feel free to join the [Heroku Cloud Native Buildpacks discussions][discussions]. You may also wish to take a look at [Heroku's product roadmap][roadmap] to see where are headed.

## Ideas and Feedback

The `heroku/ruby` buildpack is user centered software focused on application developers. To improve the application developer experience, we need to get real world feedback. We are interested in failure stories "what could be better," success stories "what went well," and your experiences in general.

Please use [Heroku Cloud Native Buildpacks discussions][discussions] to provide feedback, request enhancements, or discuss ideas.

## Issues, Feature Requests, and Bug Reports

Issues should be for bugs in buildpack behavior, and not for general "my application has a problem" support. You can share your experiences building with the heroku/ruby CNB on our [discussions][discussions]. If you're unsure about whether or not to file an issue, please review the [application contract](docs/application_contract.md).

Issues, feature requests, and bug reports are tracked via [GitHub issues on this repository][issues]. If you find an issue and/or bug, please search the issues, and if it isn't already tracked, create a new issue. Please describe the expected behavior, actual behavior, and why the two did not align. If possible, include an [example application that reproduces the problem](https://www.codetriage.com/example_app).

## Fixes, Improvements, and Patches

Fixes, improvements, and patches all happen via [GitHub Pull Requests on this repository][pulls]. If you'd like to improve the tests, you want to make the documentation clearer, you have an alternative implementation of something that may have advantages over the way its currently done, or you have any other change, we would be happy to hear about it. For trivial changes, send a pull request. For non-trivial changes, consider [opening an issue](#issues-feature-requests-and-bug-reports) to discuss it first instead.

## Development

### Dependencies

This buildpack relies on [heroku/libcnb.rs][libcnb] to compile buildpacks. All [libcnb.rs dependencies][libcnb-deps] will need to be setup prior to building or testing this buildpack.

1. Install [rust by following instructions on their site][install-rust]
1. Follow the setup instructions on [heroku/libcnb][libcnb-deps]

### Building the buildpack locally

1. Run `cargo check` to download dependencies and ensure there are no compilation issues.
1. Build the buildpack:

```
cargo libcnb package
```

### Generate an application image

Once you have have built the buildpack you can use `pack build` to generate an application image.

```
pack build sample-app \
  --buildpack packaged/x86_64-unknown-linux-musl/debug/heroku_ruby \
  --path buildpacks/ruby/tests/fixtures/default_ruby
```

This will create an image named `sample-app` based off of the fixture at `buildpacks/ruby/tests/fixtures/default_ruby`.

The deployed buildpack ships with a builder that tells the `pack` CLI what other builpacks it needs. In development you must specify them via the `--buildpack` flag before this buildpack. For example to build an app that needs nodejs can run like this:

```
pack build sample-app \
  --buildpack heroku/nodejs-engine \
  --buildpack heroku/procfile \
  --buildpack packaged/x86_64-unknown-linux-musl/debug/heroku_ruby \
  --path <path/to/application>
```

List of buildpacks this buildpack depends on:

```
--buildpack heroku/nodejs-engine
--buildpack heroku/nodejs-yarn
--buildpack heroku/jvm
```

### Run an application image

Once an image is built you can run and inspect it. Here are some example commands.

- Interactive execution

```
docker run -it --rm my-image-name bash
```

- Run a command and exit

```
docker run -it --rm my-image 'which bundle'
```

- Boot the default webserver

```
docker run -it --rm --env PORT=9292 -p 9292:9292 my-image
```

- Inspect the image:

```
pack inspect my-image
```

### Testing

- `cargo test` performs Rust unit tests.
- `cargo test -- --ignored` performs all integration tests.

See the [CI configuration](.github/workflows/ci.yml) for detailed lint and test commands.

## Code of Conduct

Please follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## License

By contributing your code, you agree to license your contribution under the terms of our project [LICENSE](LICENSE) and to sign the [Salesforce CLA](https://cla.salesforce.com/sign-cla).

[discussions]: https://github.com/heroku/buildpacks/discussions
[install-rust]: https://www.rust-lang.org/tools/install
[issues]: https://github.com/heroku/buildpacks-ruby/issues
[libcnb]: https://github.com/heroku/libcnb.rs
[libcnb-deps]: https://github.com/heroku/libcnb.rs#development-environment-setup
[pulls]: https://github.com/heroku/buildpacks-ruby/pulls
[roadmap]: https://github.com/heroku/roadmap
