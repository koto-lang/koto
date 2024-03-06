# Error Handling

Errors can be _thrown_ in the Koto runtime, which then cause the runtime to stop
execution.

A `try` / `catch` expression can be used to _catch_ any thrown errors,
allowing execution to continue. 
An optional `finally` block can be used for cleanup actions that need to 
performed whether or not an error was caught.

```koto
x = [1, 2, 3]
try
  # Accessing an invalid index will throw an error
  print x[100]
catch error 
  print "Caught an error"
finally
  print "...and finally"
check! Caught an error
check! ...and finally
```

`throw` can be used to explicity throw an error when an exceptional condition
has occurred.

`throw` accepts strings or objects that implement `@display`.

```koto
f = || throw "!Error!"

try
  f()
catch error
  print "Caught an error: '$error'"
check! Caught an error: '!Error!'
```
