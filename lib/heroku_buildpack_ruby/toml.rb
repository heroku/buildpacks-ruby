$LOAD_PATH << Pathname(__dir__).join("../../gems").glob("tomlrb-*").first.join("lib")
require 'tomlrb'

require_relative 'toml_dumper.rb'

module HerokuBuildpackRuby
  # Handles reading and writing TOML files
  #
  # Examle:
  #
  #   TOML.load(File.read("store.toml")) # => {}
  #   TOML.dump({config: true}) # => "config = true"
  #
  # For parsing we use a vendored gem "tomlrb" (https://github.com/fbernier/tomlrb). This
  # library was chosen because its parser is included in the library directly so it has no
  # external dependencies. Other toml parsing libraries have additional dependencies on
  # citrus or parselet.
  #
  # For dumping we use a vendored copy of the toml-rb dumper from https://github.com/emancu/toml-rb/blob/master/lib/toml-rb/dumper.rb
  # stored in the toml_dumper.rb file
  module TOML
    def self.load(string)
      Tomlrb.parse(string, symbolize_keys: true)
    end

    def self.dump(hash)
      TomlRB::Dumper.new(hash).toml_str
    end
  end
end
