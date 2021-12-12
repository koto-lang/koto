# os

A collection of utilities for working with the operating system.

# Reference

- [name](#name)
- [start_timer](#start_timer)
- [time](#time)
- [DateTime](#datetime)
- [DateTime.year](#datetimeyear)
- [DateTime.month](#datetimemonth)
- [DateTime.day](#datetimeday)
- [DateTime.hour](#datetimehour)
- [DateTime.minute](#datetimeminute)
- [DateTime.nanosecond](#datetimenanosecond)
- [DateTime.timestamp](#datetimetimestamp)
- [DateTime.timezone_offset](#datetimetimezone_offset)
- [DateTime.timestamp_string](#datetimetimestamp_string)
- [Timer](#timer)
- [Timer.@-](#timer-)
- [Timer.elapsed](#timerelapsed)

## name

`|| -> String`

Returns a string containing the name of the current operating system, e.g.
"linux", "macos", "windows", etc.

## start_timer

`|| -> Timer`

Returns a timer that can be used to measure how much time has passed while a
script is running.

### Example

```koto
t = os.start_timer()

# ...after some time...
print "Time taken: ${t.elapsed()}s"

t2 = os.start_timer()
print "Seconds between then and now: ${t2 - t}"
```

## time

`|| -> DateTime`

Returns a DateTime set to the current time, using the local timezone.

`|Number| -> DateTime`

Returns a DateTime set to the provided timestamp in seconds,
using the local timezone.

`|Number, Number| -> DateTime`

Returns a DateTime set to the provided timestamp in seconds,
using a time offset in seconds.

### Example

```koto
now = os.time()
# 2021-12-11 21:51:14

now.year()
# 2021

now.hour()
# 21

now.timestamp()
# 1639255874.53419
```

## DateTime

See [`os.time`](#time).

## DateTime.year

`|DateTime| -> Integer`

Returns the year component of the provided DateTime.

## DateTime.month

`|DateTime| -> Integer`

Returns the month component of the provided DateTime.

## DateTime.day

`|DateTime| -> Integer`

Returns the day component of the provided DateTime.

## DateTime.hour

`|DateTime| -> Integer`

Returns the hour component of the provided DateTime.

## DateTime.minute

`|DateTime| -> Integer`

Returns the minute component of the provided DateTime.

## DateTime.nanosecond

`|DateTime| -> Integer`

Returns the nanosecond component of the provided DateTime.

## DateTime.timestamp

`|DateTime| -> Float`

Returns the number of seconds since 00:00:00 UTC on January 1st 1970.

## DateTime.timezone_offset

`|DateTime| -> Integer`

Returns the DateTime's timezone offset in seconds.

## DateTime.timestamp_string

`|DateTime| -> String`

Returns a string representing the DateTime's timezone offset in seconds.

## Timer

See [`os.start_timer`](#start_timer).

## Timer.@-

`|Timer, Timer| -> Float`

Returns the time difference in seconds between two timers.

### Example

```koto
t1 = os.start_timer()
t2 = os.start_timer()
# t2 was started later than t1, so the time difference is positive
assert (t2 - t1) > 0
# t1 was started earlier than t2, so the time difference is negative
assert (t1 - t2) < 0
```

## Timer.elapsed

`|Timer| -> Float`

Returns the number of seconds that have elapsed since the timer was started.

### Example

```koto
t = os.start_timer()

# ...after some time...
print "Time taken: ${t.elapsed()}s"
```
