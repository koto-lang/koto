# thread

Utilities for working with threads.

# Reference

- [create](#create)
- [sleep](#sleep)
- [Thread](#thread)
- [Thread.join](#threadjoin)

## create

`|Function| -> Thread`

Creates a new thread and executes the provided function.

### See also

- [Thread](#thread)
- [Thread.join](#threadjoin)

### Example

```koto
threads = 0..4
  .each |i| thread.create(|| "thread {}".format i)
  .to_tuple()

threads
  .each |t| t.join()
  .to_tuple()
# ("thread 0", "thread 1", "thread 2", thread 3")

assert_eq data, [10..18]
```

## sleep

`|Thread, Number| -> ()`

Suspends the current thread for a specified number of seconds.

The duration must be positive and finite.

## Thread

A thread, created with [thread.create](#create).

## Thread.join

`|Thread| -> Value`

Waits for the thread to finish, and then returns the result of the thread's
function.

If the thread finished due to an error being thrown, then the error is
propagated to the joining thread.

### Example

```koto
t = thread.create || "hello"
t.join()
# hello
```
