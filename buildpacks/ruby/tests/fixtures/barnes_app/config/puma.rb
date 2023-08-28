require 'barnes'

before_fork do
  # worker specific setup

  Barnes.start # Must have enabled worker mode for this to block to be called
end

workers 2
