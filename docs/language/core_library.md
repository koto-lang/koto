# Core Library

The [_Core Library_](../core) provides a collection of fundamental functions
and values for working with the Koto language, organized within _modules_. 

```koto
# Get the size of a string
print! string.to_lowercase 'HELLO'
check! hello

# Return the first element of the list
print! list.first [99, -1, 3]
check! 99
```

Values in Koto automatically have access to their corresponding core modules 
via `.` access.

```koto
print! 'xyz'.to_uppercase()
check! XYZ

print! ['abc', 123].first()
check! abc

print! (7 / 2).round()
check! 4

print! {apples: 42, pears: 99}.contains_key 'apples'
check! true
```

## Prelude

Koto's _prelude_ is a collection of core library items that are automatically 
made available in a Koto script without the need for first calling `import`.

The modules that make up the core library are all included by default in the 
prelude. The following functions are also added to the prelude by default:

- [`io.print`](../core_lib/io#print)
- [`koto.copy`](../core_lib/koto#copy)
- [`koto.size`](../core_lib/koto#size)
- [`koto.type`](../core_lib/koto#type)
- [`test.assert`](../core_lib/test#assert)
- [`test.assert_eq`](../core_lib/test#assert-eq)
- [`test.assert_ne`](../core_lib/test#assert-ne)
- [`test.assert_near`](../core_lib/test#assert-near)

```koto
print 'io.print is available without needing to be imported'
check! io.print is available without needing to be imported
```
