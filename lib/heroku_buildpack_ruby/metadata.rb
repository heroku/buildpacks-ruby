module HerokuBuildpackRuby

  # The main interface for writing durable data to a metadata store
  #
  # Initialize using the desired backend:
  #
  #   metadata = Metadata.new(dir: cache_dir, type: Metadata::V2)
  #   metadata = Metadata.new(dir: layers_dir, type: Metadata::CNB)
  #
  # Once an instance is created you must request a named layer:
  #
  #   metadata.layer(:ruby)
  #
  # This will return a metadata instance for that layer that responds
  # to a standard interface `get`, `set`, and `fetch`:
  #
  #   metadata.layer(:ruby).set(version: "2.7.2")
  #   puts metadata.layer(:ruby).get(:version)
  #   # => "2.7.2"
  #
  # For testing you can use the MetadatNull class which behaves like
  # Metadata, but it is backed by an in-memory hash and does not persist
  # to disk.
  class Metadata
    def initialize(dir: , type: )
      @engines = Hash.new {|h, k| h[k] = type.new(dir: dir, name: k) }
    end

    def layer(key)
      @engines[key]
    end
  end

  # TODO migrate from old V2 structure to new V2 structure

  require_relative "metadata/v2.rb"
  require_relative "metadata/cnb.rb"
  require_relative "metadata/in_memory.rb"

  # Useful for for isolating behavior in unit tests
  #
  # Values set are stored as a hash but do not persist to disk
  #
  #   metadata = MetadataNull.new
  class MetadataNull < Metadata
    def initialize(dir: nil, type: Metadata::InMemory)
      super
    end
  end
end

