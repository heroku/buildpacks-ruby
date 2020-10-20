module HerokuBuildpackRuby
  # A temporary in-memory metadata store
  # mostly used for testing
  #
  # Example:
  #
  #   metadata = Metadata::InMemory.new
  #   metadata.set(:foo => "bar")
  #   metadata.get(:foo) # => "bar"
  #   metadata.fetch(:cinco) do
  #     "a good boy"
  #   end
  #   # => "a good boy"
  class Metadata
    class InMemory
      def initialize(dir: nil, name: nil)
        @metadata = {}
      end

      def exists?(key)
        @metadata.key?(key)
      end
      alias :exist? :exists?

      def get(key)
        @metadata[key]
      end

      def set(hash = {})
        @metadata.merge!(hash)

        @metadata.transform_values!(&:to_s)
        self
      end

      def fetch(key)
        @metadata[key] if @metadata.key?(key)
        value = yield

        set(key => value)
        value
      end
    end
  end
end
