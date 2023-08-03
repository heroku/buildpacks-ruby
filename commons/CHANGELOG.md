# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Breaking: `fun_run::CmdError` variants now use `NamedOutput` instead of `(String, Output)`
- Breaking: `build_output::section::Section::say` changed to `build_output::section::Section::step`
- Breaking: functions returning `Result<Output, fun_run::CmdError>` now return `Result<fun_run::NamedOutput, fun_run::CmdError>`
