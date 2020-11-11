# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "UserComms" do
    it "bundle install" do
      v2_io = StringIO.new
      cnb_io = StringIO.new
      [
        UserComms::V2.new(v2_io),
        UserComms::CNB.new(cnb_io),
        UserComms::Null.new
      ].each do |user_comms|
        user_comms.topic "Compiling Ruby/Rails"
        user_comms.info "Using Ruby version: ruby-2.7.1"
        user_comms.topic "Installing dependencies using bundler 2.1.4"
        user_comms.info "Running: BUNDLE_WITHOUT='development:test' BUNDLE_PATH=vendor/bundle BUNDLE_BIN=vendor/bundle/bin BUNDLE_DEPLOYMENT=1 bundle install -j4"
        user_comms.info <<~EOM
           Fetching gem metadata from https://rubygems.org/............
           Using rake 13.0.1
           Using concurrent-ruby 1.1.6
           Using minitest 5.14.1
           Using thread_safe 0.3.6
           Using zeitwerk 2.4.0
           Using builder 3.2.4
           Using erubi 1.9.0
           Using mini_portile2 2.4.0
           Using crass 1.0.6
           Using rack 2.2.3
           Using nio4r 2.5.4
           Using websocket-extensions 0.1.5
           Using mimemagic 0.3.5
           Bundle complete! 76 Gemfile dependencies, 146 gems now installed.
           Gems in the groups development and test were not installed.
           Bundled gems are installed into `./vendor/bundle`
        EOM
        user_comms.notice "Bundle completed (4.14s)"
        user_comms.warn_now(
          title: "Update your Ruby version",
          link: "https://devcenter.heroku.com/articles/ruby-support#supported-runtimes",
          body: <<~EOM
            There is a more recent Ruby version available for you to use:

            2.7.2

            The latest version will include security and bug fixes. We always recommend
            running the latest version of your minor release.

            Please upgrade your Ruby version.
          EOM
        )
      end
      # puts v2_io.string
      expect(v2_io.string.strip_control_codes).to eq(<<~EOM)
        -----> Compiling Ruby/Rails
               Using Ruby version: ruby-2.7.1
        -----> Installing dependencies using bundler 2.1.4
               Running: BUNDLE_WITHOUT='development:test' BUNDLE_PATH=vendor/bundle BUNDLE_BIN=vendor/bundle/bin BUNDLE_DEPLOYMENT=1 bundle install -j4
               Fetching gem metadata from https://rubygems.org/............
               Using rake 13.0.1
               Using concurrent-ruby 1.1.6
               Using minitest 5.14.1
               Using thread_safe 0.3.6
               Using zeitwerk 2.4.0
               Using builder 3.2.4
               Using erubi 1.9.0
               Using mini_portile2 2.4.0
               Using crass 1.0.6
               Using rack 2.2.3
               Using nio4r 2.5.4
               Using websocket-extensions 0.1.5
               Using mimemagic 0.3.5
               Bundle complete! 76 Gemfile dependencies, 146 gems now installed.
               Gems in the groups development and test were not installed.
               Bundled gems are installed into `./vendor/bundle`

               ## Notice: Bundle completed (4.14s)

               ## Warning: Update your Ruby version

               There is a more recent Ruby version available for you to use:

               2.7.2

               The latest version will include security and bug fixes. We always recommend
               running the latest version of your minor release.

               Please upgrade your Ruby version.

               Link: https://devcenter.heroku.com/articles/ruby-support#supported-runtimes
      EOM

      # puts cnb_io.string
      expect(cnb_io.string.strip_control_codes).to eq(<<~EOM)

        [Compiling Ruby/Rails]
        [INFO] Using Ruby version: ruby-2.7.1

        [Installing dependencies using bundler 2.1.4]
        [INFO] Running: BUNDLE_WITHOUT='development:test' BUNDLE_PATH=vendor/bundle BUNDLE_BIN=vendor/bundle/bin BUNDLE_DEPLOYMENT=1 bundle install -j4
        [INFO] Fetching gem metadata from https://rubygems.org/............
        [INFO] Using rake 13.0.1
        [INFO] Using concurrent-ruby 1.1.6
        [INFO] Using minitest 5.14.1
        [INFO] Using thread_safe 0.3.6
        [INFO] Using zeitwerk 2.4.0
        [INFO] Using builder 3.2.4
        [INFO] Using erubi 1.9.0
        [INFO] Using mini_portile2 2.4.0
        [INFO] Using crass 1.0.6
        [INFO] Using rack 2.2.3
        [INFO] Using nio4r 2.5.4
        [INFO] Using websocket-extensions 0.1.5
        [INFO] Using mimemagic 0.3.5
        [INFO] Bundle complete! 76 Gemfile dependencies, 146 gems now installed.
        [INFO] Gems in the groups development and test were not installed.
        [INFO] Bundled gems are installed into `./vendor/bundle`

        [Notice: Bundle completed (4.14s)]

        [Warning: Update your Ruby version]
        [INFO] There is a more recent Ruby version available for you to use:
        [INFO]
        [INFO] 2.7.2
        [INFO]
        [INFO] The latest version will include security and bug fixes. We always recommend
        [INFO] running the latest version of your minor release.
        [INFO]
        [INFO] Please upgrade your Ruby version.
        [INFO]
        [INFO] Link: https://devcenter.heroku.com/articles/ruby-support#supported-runtimes
      EOM
    end

    it "makes poetry" do
      v2_io = StringIO.new
      cnb_io = StringIO.new
      [
        UserComms::V2.new(v2_io),
        UserComms::CNB.new(cnb_io),
        UserComms::Null.new
      ].each do |user_comms|
        user_comms.topic("The free bird leaps")
        user_comms.info ""
        user_comms.info "on the back of the wind"
        user_comms.warn_now(
          title: "and floats downstream",
          link: "https://en.wikipedia.org/wiki/Maya_Angelou",
          body: <<~EOM
            till the current ends
            and dips their wings
            in the orange sun rays
            and dares to claim the sky
          EOM
        )
        user_comms.print_error_obj(
          BuildpackErrorNoBacktrace.new(
            title: "But a bird that stalks",
            body: <<~EOM
              down their narrow cage
              can seldom see through
              their bars of rage
              their wings are clipped and
              their feet are tied
              so they opens their throat to sing.
            EOM
          )
        )
        user_comms.warn_later(
          title: "The caged bird sings", body: <<~EOM
            with fearful trill
            of the things unknown
            but longed for still
            and his tune is heard
            on the distant hill for the caged bird
            sings of freedom
          EOM
        )
        user_comms.close
        user_comms.notice("I know why the caged bird sings")
      end
      # puts v2_io.string
      expect(v2_io.string.strip_control_codes).to eq(<<~EOM)
        -----> The free bird leaps
               on the back of the wind

               ## Warning: and floats downstream

               till the current ends
               and dips their wings
               in the orange sun rays
               and dares to claim the sky

               Link: https://en.wikipedia.org/wiki/Maya_Angelou

               ## Error: But a bird that stalks

               !
               !  down their narrow cage
               !  can seldom see through
               !  their bars of rage
               !  their wings are clipped and
               !  their feet are tied
               !  so they opens their throat to sing.
               !

               ## Warning: The caged bird sings

               with fearful trill
               of the things unknown
               but longed for still
               and his tune is heard
               on the distant hill for the caged bird
               sings of freedom

               ## Notice: I know why the caged bird sings
      EOM

      # puts cnb_io.string
      expect(cnb_io.string.strip_control_codes).to eq(<<~EOM)

        [The free bird leaps]
        [INFO] on the back of the wind

        [Warning: and floats downstream]
        [INFO] till the current ends
        [INFO] and dips their wings
        [INFO] in the orange sun rays
        [INFO] and dares to claim the sky
        [INFO]
        [INFO] Link: https://en.wikipedia.org/wiki/Maya_Angelou

        [Error: But a bird that stalks]
        [INFO] down their narrow cage
        [INFO] can seldom see through
        [INFO] their bars of rage
        [INFO] their wings are clipped and
        [INFO] their feet are tied
        [INFO] so they opens their throat to sing.

        [Warning: The caged bird sings]
        [INFO] with fearful trill
        [INFO] of the things unknown
        [INFO] but longed for still
        [INFO] and his tune is heard
        [INFO] on the distant hill for the caged bird
        [INFO] sings of freedom

        [Notice: I know why the caged bird sings]
      EOM
    end
  end
end
