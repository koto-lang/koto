# Errors

When an error is thrown by the Koto runtime, usually execution stops and the error is displayed in the console. 

A `try` / `catch` expression (with an optional `finally` block) can be used to catch any errors thrown by the Koto runtime, allowing execution to continue.

```koto
x = [1, 2, 3]
try
  # Do something that will throw an error 
  print x[100]
catch error 
  print "Caught an error"
finally
  print "...and finally"
check! Caught an error
check! ...and finally
```

`throw` can be used to throw an error from within a Koto script.

```koto
f = || throw "!Error!"

try
  f()
catch error
  print "Caught an error: '$error'"
check! Caught an error: '!Error!'
```

`throw` can be used with a String or a Map that implements `@display`.
