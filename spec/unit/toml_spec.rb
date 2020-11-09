# frozen_string_literal: true

require_relative "../spec_helper.rb"


RSpec.describe "toml" do
  it "parses toml correctly" do
    toml = <<~EOM
      [[provides]]
      name = "hello"
      [[provides]]
      name = "there"
    EOM

    actual = HerokuBuildpackRuby::TOML.load(toml)
    expect(actual).to eq({provides: [{name: "hello"}, {name: "there"}]})
  end
end
