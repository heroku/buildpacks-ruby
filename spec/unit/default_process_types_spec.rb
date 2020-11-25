# frozen_string_literal: true

require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "Default process types" do
    it "specifies console when there are no dependencies" do
      deps = Object.new
      def deps.version(name); end

      expect(
        DefaultProcessTypes.new(deps).to_h
      ).to eq({"console" => "bundle exec irb"})
    end

    it "works with rack apps" do
      deps = Object.new
      def deps.version(name)
        return Gem::Version.new("2.0") if name == "rack"
      end

      expect(
        DefaultProcessTypes.new(deps).to_h
      ).to eq(
        {
          "web" => "bundle exec rackup config.ru -p ${PORT:-5000}",
          "console" => "bundle exec irb",
        }
      )
    end

    it "works with rack apps with thin" do
      deps = Object.new
      def deps.version(name)
        return Gem::Version.new("2.0") if name == "rack"
        return Gem::Version.new("2.0") if name == "thin"
      end

      expect(
        DefaultProcessTypes.new(deps).to_h
      ).to eq(
        {
          "web" => "bundle exec thin start -R config.ru -e $RACK_ENV -p ${PORT:-5000}",
          "console" => "bundle exec irb",
        }
      )
    end

    it "works with rails 3 apps" do
      deps = Object.new
      def deps.version(name)
        return Gem::Version.new("3.0") if name == "railties"
      end

      expect(
        DefaultProcessTypes.new(deps).to_h
      ).to eq(
        {
          "web" => "bundle exec rails server -p ${PORT:-5000}",
          "console" => "bundle exec rails console",
        }
      )
    end

    it "works with rails 3 apps with thin" do
      deps = Object.new
      def deps.version(name)
        return Gem::Version.new("3.0") if name == "railties"
        return Gem::Version.new("2.0") if name == "thin"
      end

      expect(
        DefaultProcessTypes.new(deps).to_h
      ).to eq(
        {
          "web" => "bundle exec thin start -R config.ru -e $RAILS_ENV -p ${PORT:-5000}",
          "console" => "bundle exec rails console",
        }
      )
    end

    it "works with rails 4 apps" do
      deps = Object.new
      def deps.version(name)
        return Gem::Version.new("4.0") if name == "railties"
      end

      expect(
        DefaultProcessTypes.new(deps).to_h
      ).to eq(
        {
          "web" => "bin/rails server -p ${PORT:-5000} -e $RAILS_ENV",
          "console" => "bin/rails console",
        }
      )
    end

    it "works with rails 6 apps with rack" do
      deps = Object.new
      def deps.version(name)
        return Gem::Version.new("6.0") if name == "railties"
        return Gem::Version.new("6.0") if name == "rack"
      end

      expect(
        DefaultProcessTypes.new(deps).to_h
      ).to eq(
        {
          "web" => "bin/rails server -p ${PORT:-5000} -e $RAILS_ENV",
          "console" => "bin/rails console",
        }
      )
    end
  end
end

