require_relative "../spec_helper.rb"

module HerokuBuildpackRuby
  RSpec.describe "bash.rb" do
    it "uses user env" do
      user_env = Object.new
      def user_env.to_shell; %Q{WHY="theluckystiff"}; end
      def user_env.empty?; false; end

      bash = Bash.new('echo "Hello $WHY"', user_env: user_env)
      expect(bash.run.strip).to eq("Hello theluckystiff")

      bash = Bash.new('echo "Hello $WHY"', user_env: false)
      expect(bash.run.strip).to eq("Hello")
    end

    describe "on error" do
      it "can error on run!" do
        bash = Bash.new("echo 'nope'; exit1")
        expect {
          bash.run!
        }.to raise_error(/Bash command failed/)
      end

      it "returns the original command" do
        bash = Bash.new("echo 'nope'; exit 1")
        expect {
          bash.run!
        }.to raise_error(/echo 'nope'; exit 1/)
      end

      it "redacts env vars" do
        user_env = Object.new
        def user_env.to_shell; %Q{WHY="theluckystiff"}; end
        def user_env.empty?; false; end

        bash = Bash.new("echo 'nope'; exit 1", user_env: user_env)
        expect {
          bash.run!
        }.to raise_error(/<REDACTED> bash -c echo/)

        expect(bash.command_without_env).to(
          eq("/usr/bin/env <REDACTED> bash -c echo\\ \\'nope\\'\\;\\ exit\\ 1 2>&1")
        )
      end
    end
  end
end
