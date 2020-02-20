if exists("b:current_syntax")
  finish
endif

syntax keyword holzTodos contained TODO FIXME NOTE
syntax keyword holzConditionals if then else
syntax keyword holzRepeating for in
syntax keyword holzBuiltins print length push
syntax keyword holzAsserts assert
syntax match holzCapture "\v\|"

syntax match holzInlineComment "#.*$"
  \ contains=holzTodos oneline
syntax region holzMultilineComment start="#-" end="-#"
      \ contains=holzTodos,holzMultilineComment fold

syntax keyword holzOperator and or
syntax match holzOperator "\v\+"
syntax match holzOperator "\v\-"
syntax match holzOperator "\v\*"
syntax match holzOperator "\v\/"
syntax match holzOperator "\v\>"
syntax match holzOperator "\v\<"
syntax match holzOperator "\v\="

syntax region holzString start=/"/ end=/"/ oneline

syntax keyword holzBoolean true false
syntax match holzNumber "\v<\d+>"
syntax match holzNumber "\v<(\d+_+)+\d+(\.\d+(_+\d+)*)?>"
syntax match holzNumber "\v<\d+\.\d+>"
syntax match holzNumber "\v<\d*\.?\d+([Ee]-?)?\d+>"



highlight default link holzInlineComment Comment
highlight default link holzMultilineComment Comment

highlight default link holzTodos Todo
highlight default link holzConditionals Conditional
highlight default link holzRepeating Repeat
highlight default link holzBuiltins Function
highlight default link holzAsserts Macro
highlight default link holzCapture Keyword
highlight default link holzOperator Operator

highlight default link holzBoolean Boolean
highlight default link holzNumber Number
highlight default link holzString String
