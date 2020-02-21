if exists("b:current_syntax")
  finish
endif

syntax keyword songTodos contained TODO FIXME NOTE
syntax keyword songConditionals if then else
syntax keyword songRepeating for in
syntax keyword songBuiltins print length push
syntax keyword songAsserts assert
syntax match songCapture "\v\|"

syntax match songInlineComment "#.*$"
  \ contains=songTodos oneline
syntax region songMultilineComment start="#-" end="-#"
      \ contains=songTodos,songMultilineComment fold

syntax keyword songOperator and or
syntax match songOperator "\v\+"
syntax match songOperator "\v\-"
syntax match songOperator "\v\*"
syntax match songOperator "\v\/"
syntax match songOperator "\v\>"
syntax match songOperator "\v\<"
syntax match songOperator "\v\="

syntax region songString start=/"/ end=/"/ oneline

syntax keyword songBoolean true false
syntax match songNumber "\v<\d+>"
syntax match songNumber "\v<(\d+_+)+\d+(\.\d+(_+\d+)*)?>"
syntax match songNumber "\v<\d+\.\d+>"
syntax match songNumber "\v<\d*\.?\d+([Ee]-?)?\d+>"



highlight default link songInlineComment Comment
highlight default link songMultilineComment Comment

highlight default link songTodos Todo
highlight default link songConditionals Conditional
highlight default link songRepeating Repeat
highlight default link songBuiltins Function
highlight default link songAsserts Macro
highlight default link songCapture Keyword
highlight default link songOperator Operator

highlight default link songBoolean Boolean
highlight default link songNumber Number
highlight default link songString String
