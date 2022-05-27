# Modules

## `import`

Module items can be brought into the current scope using `import`.

```koto
import list.last, number.abs
x = [1, 2, 3]
print! last x
check! 3

print! abs -42
check! 42
```

Multiple items from a single module can be imported using `from`.

```koto
from tuple import first, last, size
x = 'a', 'b', 'c'
print! first x
check! a
print! last x
check! c
print! size x
check! 3
```

Imported items can be assigned to alternative names.

```koto
list_size = import list.size
tuple_size = import tuple.size
print! list_size [1, 2]
check! 2
print! tuple_size (3, 2, 1)
check! 3
```

## `export`

`export` is used to add a value to a module's _exports map_.

```koto,skip_run
##################
# my_module.koto #
##################

export say_hello = |name| 'Hello, $name!'

##################
##################

import my_module.say_hello

say_hello 'Koto'
check! 'Hello, Koto!' 
```

## `@tests` and `@main`

A module can export a `@tests` Map containing `@test` functions, which will be 
run after the module has been compiled and initialized.

Additionally, a module can export a `@main` function. 
The `@main` function will be called after the module has been compiled and
initialized, and after exported `@tests` have been successfully run.

Note that because meta entries can't be directly accessed after assignment,
adding an entry to the module's Meta Map doesn't require `export`.

```koto,skip_run
##################
# my_module.koto #
##################

export say_hello = |name| 'Hello, $name!'

@main = ||
  print 'Successfully initialized `my_module`'

@tests = 
  @test hello_world: ||
    print 'Testing...'
    assert_eq (say_hello 'World'), 'Hello, World!'

##################
##################

import my_module.say_hello
check! Testing...
check! Successfully initialized `my_module`

say_hello 'Koto'
check! 'Hello, Koto!' 
```

## Module Paths

By default `import` will look for a `.koto` file
with a matching name, or for a folder with a matching name containing a
`main.koto` file.

e.g. If an `import foo` expression is encountered by the runtime, 
then a `foo.koto` file will be looked for in the same location as the current
script, and if not found then `foo/main.koto` will be checked for.
