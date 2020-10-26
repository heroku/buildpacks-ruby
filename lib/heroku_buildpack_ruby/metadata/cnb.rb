module HerokuBuildpackRuby
  class Metadata

    # An interface for a durable metadata store for CNB (bin/build)
    #
    # Will persist values to a CNB layer's `store.toml` durably
    # so that it can persist between builds.
    #
    # Example:
    #
    #   layers_dir = Dir.pwd
    #   metadata = Metadata::CNB.new(dir: layers_dir, name: :ruby)
    #   metadata.set(:foo => "bar")
    #   metadata.get(:foo) # => "bar"
    #   metadata.fetch(:cinco) do
    #     "a good boy"
    #   end
    #   # => "a good boy"
    class CNB
      def initialize(dir: ,name:)
        @store = Pathname(dir).join(name.to_s, "store.toml").tap {|p| p.dirname.mkpath; FileUtils.touch(p) }
        read
      end

      def exists?(key)
        @metadata.key?(key)
      end
      alias :exist? :exists?

      def get(key)
        @metadata[key]
      end

      def set(hash={})
        hash.each do |k, v|
          @metadata[k] = v
        end
        write

        self
      end

      def fetch(key)
        return @metadata[key] if @metadata.key?(key)

        value = yield
        set(key => value)
        write

        value
      end


      def to_h
        @metadata.dup
      end

      private def write
        @toml[:metadata] = @metadata
        @store.write TOML.dump(@toml)
      end

      private def read
        @toml = TOML.load(@store.read) || {}
        @metadata = @toml[:metadata] || {}
      end
    end
  end
end
