# Errors

`try`, `catch`, and `finally` can be used to catch any errors thrown by the Koto runtime.

```koto
x = [1, 2, 3]
try
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
  print "Caught an error: $error"
check! Caught an error: !Error!
```

