# frozen_string_literal: true

require_relative "../spec_helper.rb"

RSpec.describe "metadata" do
  describe "top level interface" do
    it "works" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)
        metadata = HerokuBuildpackRuby::Metadata.new(dir: dir, type: HerokuBuildpackRuby::Metadata::CNB)
        expect(metadata.layer(:ruby).class).to eq(HerokuBuildpackRuby::Metadata::CNB)

        metadata = HerokuBuildpackRuby::Metadata.new(dir: dir, type: HerokuBuildpackRuby::Metadata::V2)
        expect(metadata.layer(:ruby).class).to eq(HerokuBuildpackRuby::Metadata::V2)
      end
    end

    it "null can be created with no directory or type" do
      null = HerokuBuildpackRuby::Metadata::Null.new
      null.layer(:ruby).set(version: "2.7.2")
      expect(null.layer(:ruby).get(:version)).to eq("2.7.2")
    end
  end

  describe "in memory Engine" do
    it "works" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)
        engine = HerokuBuildpackRuby::Metadata::InMemory.new(dir: dir, name: :ruby)
        expect(engine.get(:foo)).to be_nil
        engine.set(foo: "bar")
        expect(engine.get(:foo)).to eq("bar")

        expect(engine.get(:lol)).to eq(nil)
        out = engine.fetch(:lol) do
          "haha"
        end
        expect(out).to eq("haha")
        expect(engine.get(:lol)).to eq("haha")

        out = engine.fetch(:lol) do
          "hehe"
        end
        expect(out).to eq("haha")
        expect(engine.get(:lol)).to eq("haha")
      end
    end
  end

  describe "CNB Engine" do
    it "works" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)
        engine = HerokuBuildpackRuby::Metadata::CNB.new(dir: dir, name: :ruby)
        expect(engine.get(:foo)).to be_nil
        engine.set(foo: "bar")
        expect(engine.get(:foo)).to eq("bar")

        expect(engine.get(:lol)).to eq(nil)
        out = engine.fetch(:lol) do
          "haha"
        end
        expect(out).to eq("haha")
        expect(engine.get(:lol)).to eq("haha")

        out = engine.fetch(:lol) do
          "hehe"
        end
        expect(out).to eq("haha")
        expect(engine.get(:lol)).to eq("haha")

        hash = HerokuBuildpackRuby::TOML.load(dir.join("ruby", "store.toml").read)
        expect(hash).to eq({:metadata=>{:foo=>"bar", :lol=>"haha"}})


        engine_2 = HerokuBuildpackRuby::Metadata::CNB.new(dir: dir, name: :ruby)

        expect(engine_2.to_h).to eq(engine.to_h)
      end
    end
  end

  describe "V2 Engine" do
    it "works" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)
        engine = HerokuBuildpackRuby::Metadata::V2.new(dir: dir, name: :ruby)
        expect(engine.get(:foo)).to be_nil
        engine.set(foo: "bar")
        expect(engine.get(:foo)).to eq("bar")

        expect(engine.get(:lol)).to eq(nil)
        out = engine.fetch(:lol) do
          "haha"
        end
        expect(out).to eq("haha")
        expect(engine.get(:lol)).to eq("haha")

        out = engine.fetch(:lol) do
          "hehe"
        end
        expect(out).to eq("haha")
        expect(engine.get(:lol)).to eq("haha")

        expect(dir.join("ruby").entries.map(&:to_s)).to include("lol")
        expect(dir.join("ruby").entries.map(&:to_s)).to include("foo")

        expect(dir.join("ruby", "foo").read).to eq("bar")
        expect(dir.join("ruby", "lol").read).to eq("haha")


        engine_2 = HerokuBuildpackRuby::Metadata::V2.new(dir: dir, name: :ruby)
        expect(engine_2.to_h).to eq(engine.to_h)
      end
    end
  end
end
