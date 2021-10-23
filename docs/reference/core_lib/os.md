# os

A collection of utilities for working with the operating system.

# Reference

- [cpu_count](#cpu_count)
- [name](#name)
- [physical_cpu_count](#physical_cpu_count)

## cpu_count

`|| -> Int`

Provides the number of logical CPU cores that are available in the system.

Note that this may differ from the number of physical CPU cores in the system,
which is provided by [physical_cpu_count](#physical_cpu_count).

## name

`|| -> String`

Returns a string containing the name of the current operating system, e.g.
"linux", "macos", "windows", etc.

## physical_cpu_count

`|| -> Int`

Provides the number of physical CPU cores that are available in the system.

Note that this may differ from the number of logical CPU cores in the system,
which is provided by [cpu_count](#cpu_count).
