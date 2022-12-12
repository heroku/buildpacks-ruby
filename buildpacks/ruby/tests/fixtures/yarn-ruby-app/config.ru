ruby_version = `ruby -v`

run proc {|env| [200, {'Content-Type' => 'text/plain'}, [ruby_version]] }
