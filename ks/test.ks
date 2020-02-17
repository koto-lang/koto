# This is a comment

print "Hello, World!" # This is a trailing comment

###### Basic types and logic
a = true
print a
assert a == true

###### Functions

# Functions are defined with the -> operator
say_hi = -> print "Hi!"
say_hi()

# # Arguments come before ->
# # Implicit return of last statement in function
square = x -> x * x

# # Muliple-argument functions must be called with parentheses
# # Single-argument functions can be called with or without parentheses
print("The square of 7 is", square(7))

# Multiple arguments are separated by commas
# Multiline functions are indented after the -> operator
# Functions can be nested - TODO args should be local
add = x, y ->
  do_add = x, y ->
    x + y
  result = do_add(x, y)
  print(x, "+", y, "==", result)
  result

assert add(10, 20) == 30

# Arrays
# z = [10, 20, 30]
# assert z[0] == 10


# # Loops
# evens = [0, 2, 4, 6, 8]
# for x in evens
#   print x

# for (i, x) in enumerate b
#   print format("{}: {}", i, x)

# c = [square(x) for x in b]

# ###### Ranges

# # Ranges are lazily evaluated
# z = 0..20
# # Anonymous functions can be passed as parameters
# # [] collects a range into an array
# a = [filter(z, x -> x < 10)]
# assert(length(a) == 10)
# # ..= creates an inclusive range
# y = [1..=5]
# assert(length(y) == 5)


# ###### Tables
# o = { min: 0, max: 42 }
# sum = 0
# for i in o.min..o.max
#   sum = sum + i

# o = {
#   bar: x -> square x
# }

# x = o.bar(9)
# o = o + { baz = 99 }

# ##### Classes
# class O
#   new => self.foo = 42
#   with_foo x ->
#     o = new O
#     o.foo = x
#     o
#   print_foo => print self.foo

# o = new O
# o.print_foo()

# ##### Standard Library
# x = math.sin 42
