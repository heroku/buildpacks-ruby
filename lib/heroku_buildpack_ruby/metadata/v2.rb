# frozen_string_literal: true

module HerokuBuildpackRuby
  class Metadata

    # An interface for a durable metadata store for legacy/V2 (bin/compile)
    #
    # Will persist values to a directory using keys as filenames and
    # contents as their values.
    #
    # Example:
    #
    #   cache_dir = Dir.pwd
    #   metadata = Metadata::V2.new(dir: cache_dir, name: :ruby)
    #   metadata.layer(:ruby).set(:foo => "bar")
    #   metadata.layer(:ruby).get(:foo) # => "bar"
    #   metadata.layer(:ruby).fetch(:cinco) do
    #     "a good boy"
    #   end
    #   # => "a good boy"
    class V2
      def initialize(dir: ,name:)
        @dir = Pathname(dir).join(name.to_s).tap(&:mkpath)
        @metadata = {}

        @dir.entries.each do |entry|
          path = @dir.join(entry)
          next if path.directory?

          @metadata[entry.to_s.to_sym] = path.read
        end
      end

      def exists?(key)
        @metadata.key?(key)
      end
      alias :exist? :exists?

      def get(key)
        @metadata[key]
      end

      def set(hash = {})
        hash.each do |k, v|
          @metadata[k] = v
          @dir.join(k.to_s).write v.to_s
        end
        self
      end

      def fetch(key)
        return @metadata[key] if @metadata.key?(key)

        value = yield
        set(key => value)

        value
      end

      def to_h
        @metadata.dup
      end
    end
  end
end
