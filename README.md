## Application contract

- We will request node to be installed via the heroku/nodejs buildpack on your system when `package.json` is found but `which node` is not present
  - See heroku/nodejs for their application contract
  - [TODO] https://github.com/heroku/heroku-buildpack-ruby-experimental/issues/27
- We will request java to be installed via the heroku/jvm buildpack on your system when your Gemfile.lock specifies jruby but `which java` is not present
  - See heroku/jvm for their application contract
  - [TODO] https://github.com/heroku/heroku-buildpack-ruby-experimental/issues/36
- We will determine a version of bundler for you based on the contents of your Gemfile.lock. You cannot specify the exact version, just major version i.e. 1.x or 2.x.
- We will determine your requested version of Ruby using `bundle platform --ruby` (or similar logic).
- We will install your gem dependencies using `bundle install`.
- We will run `bundle clean` after `bundle install` and before caching
  - We will cache the contents of your gem dependencies
    - We will invalidate the dependency cache if your Ruby version changes [TODO]
    - We will invalidate the dependency cache if your stack changes [TODO]
    - We may invalidate the dependency cache if there was a bug in a prior buildpack version that needs to be fixed [TODO]
- We will parse your Gemfile.lock to determine what dependencies your app need for use in specializing your install behavior (i.e. Rails 5 versus Rails 4 etc.).
- We will determine what rake tasks you have available via the output of `rake -P` against your application.
  - We require applications have a version of `rake` in the Gemfile.lock and a Rakefile variant at the root of their application.
  - We may error out if this command fails based on your dependencies.
- We will run `rake assets:precompile` on your app if it exists on your application.
  - We will skip this task if a manifest file exists in the `public/assets` folder that indicates precompiled assets are checked into git.
  - We will run `rake assets:clean` on your app.
    - We will cache the contents of `public/assets` if `assets:clean` exists on your application. [TODO]
    - We will limit or prune the size of this asset cache [TODO]
    - We will cache asset "fragments" directories if the `sprockets` gem is on the system [TODO]

- We will set a default "console" process type based on the contents of your Gemfile.lock
  - Use the Procfile to override
- We will set a default "web" process type based on the contents of your Gemfile.lock
  - Use the Procfile to override

Goal: Convert all "may" statements to a more specific "will" so we're explicit about when thing shappen

## Internal Concepts

If you want to work on this project here are some introduction explanations for several concepts that might be confusing:

### Ruby dependencies

The environment that the Ruby buildpack executes on is minimal and it is intended for an application target, not the Ruby buildpack target. Due to this limitation we want to prefer using as few dependencies as possible, and as little tooling at execution time. This does not mean you cannot use dependencies inside of the buildpack, but it means you must manuallyvendor and manage them. Gems go in the `gems` directory:

- $ gem install tomlrb --install-dir ./ --no-document

Now it can be manually required. After adding the appropriate directory to the load path.

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

### Initialize values, call behavior

In general it's prefered to initialize all values when creating an object rather than passing values in later. We also want to decouple actions from initialization. The pattern here is to have actions respond when executintg `call` on the object. In general it allows us to be flexible with when we create our objets and when we use them.

### Private by default

Methods and accessors should be private when possible. This allows for a more explicit API. An explicit API provides more room for future refacoring and changes while minimizing changes needed to other classes. You can make attr_ methods private like this:

```ruby
class Whatever
  private; attr_reader :foo, :bar; public
end
```

### attr_reader for values that may be falsey

When working with a variable that may be falsey, prefer to use an `attr_reader` over accessing the instance variable directly. This will guard against spelling mistakes. For example:

```ruby
class Foo
  private; attr_reader :value; public

  def initialize(value)
    @value = value
  end

  def correct_will_raise_an_error_due_to_misspelling
    puts "value is set" if valueee
  end

  def incorrect_will_not_raise_an_error_due_to_misspelling
    puts "value is set" if @valueee
  end
end
```

For other cases where methods are called on the object being passed in, it's less important.

### Pass values over behavior

This could also be "dependency inversion" if you like jargon. Basically if your class needs to call a method to get a value from another object, instead of passing the object, pass the value:

```ruby

class AssetsPrecompileTooManyDependencies
  def initialize(rake:)
    @has_assets_precompile = rake.detect?("assets:precompile")
  end
end
AssetsPrecompileTooManyDependencies.new(rake: rake)


class AssetsPrecompileValuesOnly
  def initialize(has_assets_precompile:)
    @has_assets_precompile = has_assets_precompile
  end
end

AssetsPrecompileValuesOnly.new(has_assets_precompile: rake.detect?("assets:precompile"))
```

This makes it easier to test since we can test the test the asset precompile logic without also needing to invoke a mock or stub of the rake object. Sometimes classes do need to know about other classes, that's fine when it's needed, but try to pass values when possible.

### Anounce conditionals to the user

If you're going to have different behavior, then tell your user what the difference is and why. For example, don't just "not" load a cache, anounce that the cache will not be loaded due to {reason}. This has the knock on effect of effectively saying almost every 'if' branch also needs an `else` codepath.

### Avoid nested conditionals if possible

This is a personal preference. One nesting is fine, more than that and the logic can get hairy. Consider early returns or moving conditional logic to other parts of code (such as early returns, case statements, or is/elsif.

### Dependency injection for all puts/print/user communication

Instead of outputting to STDOUT or STDERR directly, wrap all communications in an interface that can be injected. This allows capturing that output for tests, as well as for minimizing "stuff in my dots" while unit tests are running.

## External concepts

These are external concepts that you may run into while working in this codebase. They're not always immediately self-explanatory, this section might help to understand their intended use cases.

### Bundler.with_original_env do

This invocation is frequently used in tests. It essentially tells bundler to set environment variables back to what they were before bundler was invoked. This is needed when we are wanting to simulate running a ruby command from the command line without bundler invoked yet. It's not always needed in every context, but if you see it, now you know why it's there and what the goal is.

### Ideas

- A "HEROKU_DEBUG_DEPLOY" mode that stops execution but still attempts to write out env vars for what it's got so far so that the results on disk can be interactively inspected
- The ability to record and replay the actions of the buildpack as bash commands. This could be useful for reproducing and reporting issues for maintainers of other projects, for example we could give rubygems or bundler a repro with docker and bash commands.
