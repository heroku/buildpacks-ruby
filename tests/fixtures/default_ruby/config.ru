require 'json'

ruby_version = `ruby -v`.strip
which_ruby = `which ruby`.strip

payload = %Q[{"ruby_version": "#{ruby_version}", "which_ruby": "#{which_ruby}"}]
run proc {|env| [200, {'Content-Type' => 'text/plain'}, [payload]] }
