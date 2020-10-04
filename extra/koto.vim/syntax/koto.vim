if exists("b:current_syntax")
  finish
endif

syntax keyword kotoTodos contained TODO FIXME NOTE
syntax keyword kotoKeywords
  \ catch copy debug export finally from import not num2 num4 return self try type yield
syntax keyword kotoConditionals if else match then
syntax keyword kotoRepeating break continue for in loop until while
syntax keyword kotoStdLib
  \ env io json list map math print push string test thread toml
syntax keyword kotoAsserts assert assert_eq assert_ne assert_near
syntax match kotoCapture "\v\|"

syntax match kotoInlineComment "#.*$"
  \ contains=kotoTodos oneline
syntax region kotoMultilineComment start="#-" end="-#"
  \ contains=kotoTodos,kotoMultilineComment fold

syntax keyword kotoOperator and or
syntax match kotoOperator "\v\+"
syntax match kotoOperator "\v\-"
syntax match kotoOperator "\v\*"
syntax match kotoOperator "\v\/"
syntax match kotoOperator "\v\%"
syntax match kotoOperator "\v\>"
syntax match kotoOperator "\v\<"
syntax match kotoOperator "\v\="

syntax region kotoString start=/"/ end=/"/ oneline

syntax keyword kotoBoolean true false
syntax match kotoNumber "\v<\d+>"
syntax match kotoNumber "\v<(\d+_+)+\d+(\.\d+(_+\d+)*)?>"
syntax match kotoNumber "\v<\d+\.\d+>"
syntax match kotoNumber "\v<\d*\.?\d+([Ee]-?)?\d+>"



highlight default link kotoInlineComment Comment
highlight default link kotoMultilineComment Comment

highlight default link kotoTodos Todo
highlight default link kotoKeywords Keyword
highlight default link kotoConditionals Conditional
highlight default link kotoRepeating Repeat
highlight default link kotoStdLib Function
highlight default link kotoAsserts Macro
highlight default link kotoCapture Type
highlight default link kotoOperator Operator

highlight default link kotoBoolean Boolean
highlight default link kotoNumber Number
highlight default link kotoString String
