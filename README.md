

## Internal Concepts

If you want to work on this project here are some introduction explanations for several concepts that might be confusing:

### V2 and CNB support

This buildpack supports both the Heroku legacy `/bin/compile` interface, also known as "v2" as well as the newer specification for Cloud Native Buildpacks (CNB). Some concepts present in one interface are not used at all in the other (for instance "v2" has no concept of "layers". For more info see their respective docs:

- [CNB spec](https://github.com/buildpacks/spec)
- [V2 - Heroku's legacy spec](https://devcenter.heroku.com/articles/buildpack-api)

### EnvProxy

Environment variable mutation should be done through EnvProxy instances which record the mutation so it can be replayed back to export files. For more information see the docs in [env_proxy.rb]().

### UserOutput

Communication to the user should be directed through a UserOutput object. This way multiple backends (V2/CNB) can be switched and we can change one without having to chane the other.

## Coding Conventions

Here's some conventions that this project uses. Stick to them when it makes sense, but they're not rules: break them when it makes the code better in a defensible way.

### Unit testing

The ability to split appart and unit test individual behaviors aids in a fast response cycle for rapid buildpack development. They're not comprehensive, but can help to guide changes without requiring a long deployment cycle.

Structure of features revolves around classes that can be used independently of the buildpack (so they can be unit tested). In the "fat model/skinny controller" paradigm think of the main entry point to HerokuBuildpackRuby as a controller, and individual classes as models. We want to compose as much behavior as possible.

### Naming conventions

Files and directories should be represented as Pathname objects. Objects representing a specific file should use variables ending in `*_path` or `*_file` while variables representing directories should end in `*_dir`. If multiple things have similar names, try to put the different bit up front. I.e.

Instead of "version_ruby" and "version_bundler" consider "ruby_version" and "ruby_bundler". This applies to file and class naming as well.

### Mixins and inheritance are discouraged

Share behavior by sharing objects when possible. Relatedly: Strive for DRY concepts, not DRY source code.

## Initialize values, call behavior

In general it's prefered to initialize all values when creating an object rather than passing values in later. We also want to decouple actions from initialization. The pattern here is to have actions respond when executintg `call` on the object. In general it allows us to be flexible with when we create our objets and when we use them.
