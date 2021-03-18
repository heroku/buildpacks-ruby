# frozen_string_literal: true

module HerokuBuildpackRuby
  # Calls `rake assets:precompile`
  #
  # Example:
  #
  #   app_dir = Pathname(Dir.pwd)
  #   HerokuBuildpackRuby::AssetsPrecompile.new(
  #     app_dir: app_dir,
  #     has_assets_clean: rake.detect?("assets:clean"),
  #     has_assets_precompile: rake.detect?("assets:precompile"),
  #   ).call
  #
  #   public_dir = app_dir.join("public/assets")
  #   puts public_dir.directory? => true
  #   puts public_dir.empty? => false
  class AssetsPrecompile
    private; attr_reader :user_comms, :has_assets_precompile, :has_assets_clean; public

    def initialize(has_assets_precompile:, has_assets_clean: , app_dir: , user_comms: UserComms::Null.new)
      @has_assets_precompile = has_assets_precompile
      @has_assets_clean = has_assets_clean
      @user_comms = user_comms
      @public_dir = Pathname(app_dir).join("public/assets")
    end

    def call
      warn_assets_precompiled_maniefest and return self if assets_manifest

      assets_precompile
      assets_clean
      self
    end

    def precompile_fail(output:, body: String.new, link: nil)
      if output.match(/(127\.0\.0\.1)|(org\.postgresql\.util)/)
        body << "Attempted to access a nonexistent database"
        link = "https://devcenter.heroku.com/articles/pre-provision-database"
      end

      body << output

      user_comms.error(
        title: "Precompiling assets failed",
        body: body,
        link: link,
      )
    end

    private def warn_assets_precompiled_maniefest
      @user_comms.info("Skipping `rake assets:precompile`: Precompiled asset manifest found: #{assets_manifest}")
    end

    private def assets_precompile
      if has_assets_precompile
        user_comms.topic("Running: rake assets:precompile")
        task = RakeTask.new("assets:precompile", stream: @user_comms).call

        precompile_fail(output: task.out) if task.fail?
      else
        user_comms.info("Asset compilation skipped: `rake assets:precompile` not found")
      end
    end

    private def assets_clean
      if has_assets_clean
        user_comms.topic("Running: rake assets:clean")

        RakeTask.new("assets:clean", stream: @user_comms).call
      else
        user_comms.info("Asset clean skipped: `rake assets:clean` not found")
      end
    end

    private def assets_manifest
      @public_dir.glob(manifest_glob_pattern).first
    end

    private def manifest_glob_pattern
      files_string = [".sprockets-manifest-*.json", "manifest-*.json", "manifest.yml"].join(",")
      "{#{files_string}}"
    end
  end
end
