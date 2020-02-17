# This is a comment

####
# This is a
# multline comment
####

### This is not a multiline comment

print("Hello, World!")

# ###### Basic types and logic
# a = true
# assert a == true

# a = false
# assert a != true

# ###### Functions

# # Functions are defined with the -> operator
# say_hi ->
#   print "Hello, World!"
# say_hi()

# # Arguments come before ->
# # Implicit return of last statement in function
# square x -> x * x
# # Muliple-argument functions must be called with parentheses
# # Single-argument functions can optionally be called without parentheses
# print("The square of 7 is", square 7)

# # Multiple arguments are separated by spaces
# add x y -> x + y
# a = 2.5
# b = add(a, 9 / 3)
# print(b, -1.0, "Third")

# # Loops and Arrays
# for i in 0..10
#   print i

# b = [0, 2, 4, 6, 8]
# for x in b
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

