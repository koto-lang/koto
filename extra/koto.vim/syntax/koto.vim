if exists("b:current_syntax")
  finish
endif

syntax keyword kotoTodos contained TODO FIXME NOTE
syntax keyword kotoKeywords copy debug export not num4 return self
syntax keyword kotoConditionals if then else
syntax keyword kotoRepeating break continue for in until while
syntax keyword kotoStdLib env io list map math number print push size string thread
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
