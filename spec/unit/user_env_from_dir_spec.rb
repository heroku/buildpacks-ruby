require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "UserEnvFromDir" do
    it "handles empty contents" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)

        env = UserEnvFromDir.new.parse(dir)

        expect(env.key?("FOUR")).to be_falsey
        expect(env["FOUR"]).to eq(nil)
        expect(env.to_shell).to eq(%Q{})
      end
    end

    it "reads from the env dir" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)
        dir.join("FOUR").write("seasons")
        dir.join("TOTAL").write("landscaping")

        env = UserEnvFromDir.new.parse(dir)

        expect(env.key?("FOUR")).to be_truthy
        expect(env["FOUR"]).to eq("seasons")
        expect(env.to_shell).to eq(%Q{FOUR="seasons" TOTAL="landscaping"})
      end
    end

    it "has a deny list" do
      Dir.mktmpdir do |dir|
        dir = Pathname(dir)
        dir.join("PATH").write("lol")

        env = UserEnvFromDir.new.parse(dir)

        expect(env.key?("PATH")).to be_falsey
      end
    end
  end
end
