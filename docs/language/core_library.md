# Core Library

The [_Core Library_](../core) provides a collection of fundamental functions
and values for working with the Koto language, organized within _modules_. 

```koto
# Get the size of a string
print! string.size 'hello'
check! 5

# Return the first element of the list
print! list.first [99, -1, 3]
check! 99
```

Values in Koto automatically have access to their corresponding core modules 
via `.` access.

```koto
print! 'xyz'.size()
check! 3

print! ['abc', 123].first()
check! abc

print! (7 / 2).round()
check! 4

print! {apples: 42, pears: 99}.contains_key 'apples'
check! true
```
