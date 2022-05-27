# Core Library

Koto includes a [Core Library](../../core) of useful functions and values organized into _modules_. 
Modules in Koto are simply Maps.

```koto
print! string.size 'hello'
check! 5

print! list.first [99, -1, 3]
check! 99
```

Values in Koto automatically have their corresponding core library modules available via `.` access.

```koto
print! 'xyz'.size()
check! 3

print! ['abc', 123].first()
check! abc

print! (11 / 2).round()
check! 6

print! {apples: 42, pears: 99}.contains_key 'apples'
check! true
```

