# Modules

Koto includes a module system that helps you to organize and re-use your code 
when your program grows too large for a single file.

## `import`

Items from modules can be brought into the current scope using `import`.

```koto
from list import last
from number import abs

x = [1, 2, 3]
print! last x
check! 3

print! abs -42
check! 42
```

Multiple items from a single module can be imported at the same time.

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

Imported items can be renamed using `as` for clarity or to avoid conflicts.

```koto
from list import size as list_size
from tuple import size as tuple_size
print! list_size [1, 2]
check! 2
print! tuple_size (3, 2, 1)
check! 3
```

## `export`

`export` is used to add values to the current module's _exports map_.

Single values can be assigned to and exported at the same time:

```koto,skip_run
##################
# my_module.koto #
##################

export say_hello = |name| 'Hello, $name!'

##################
##################

from my_module import say_hello

say_hello 'Koto'
check! 'Hello, Koto!' 
```

When exporting multiple values, it can be convenient to use map syntax:

```koto,skip_run
##################
# my_module.koto #
##################

# Define some local values
a, b, c = 1, 2, 3

# Inline maps allow for shorthand syntax
export { a, b, c, foo: 42 }

# Map blocks can also be used with export
export 
  bar: 99
  baz: 'baz'
```

## `@tests` and `@main`

A module can export a `@tests` object containing `@test` functions, which 
will be automatically run after the module has been compiled and initialized.

Additionally, a module can export a `@main` function. 
The `@main` function will be called after the module has been compiled and
initialized, and after exported `@tests` have been successfully run.

Note that because metakeys can't be assigned locally, 
the use of `export` is optional when adding entries to the module's metamap.

```koto,skip_run
##################
# my_module.koto #
##################

export say_hello = |name| 'Hello, $name!'

@main = || # Equivalent to export @main =
  print '`my_module` initialized'

@tests =
  @test hello_world: ||
    print 'Testing...'
    assert_eq (say_hello 'World'), 'Hello, World!'

##################
##################

from my_module import say_hello
check! Testing...
check! Successfully initialized `my_module`

say_hello 'Koto'
check! 'Hello, Koto!' 
```

## Module Paths

When looking for a module, `import` will look for a `.koto` file with a matching 
name, or for a folder with a matching name that contains a `main.koto` file.

e.g. When an `import foo` expression is run, then a `foo.koto` file will be 
looked for in the same location as the current script, 
and if `foo.koto` isn't found then the runtime will look for `foo/main.koto`.
