if exists("b:current_syntax")
  finish
endif

syntax keyword kotoTodos contained TODO FIXME NOTE
syntax keyword kotoKeywords
  \ catch copy debug export finally from import not num2 num4 return self try yield
syntax keyword kotoConditionals if else match then
syntax keyword kotoRepeating break continue for in loop until while

syntax keyword kotoCoreLibModules
  \ koto io iterator list map number range string thread test tuple
syntax keyword kotoCoreLib
  \ contains[] create get insert is_empty iter remove size sum
  \ args current_dir script_dir script_path type
  \ exists open path read_to_string remove_file seek temp_dir write write_line
  \ consume count each enumerate fold[] keep next take to_list to_map to_tuple zip
  \ fill first last pop push resize retain reverse sort sort_copy transform with_size
  \ contains_key keys values
  \ abs acos asin atan ceil clamp cos cosh degrees exp exp2 floor log10 log2 ln max min
  \ pi pow radians recip sin sinh sqrt tan tanh tau
  \ sleep
  \ escape format lines print slice split to_number trim
  \ end start

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

syntax region kotoString start=/"/ end=/"/

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

highlight default link kotoCoreLibModules Function
highlight default link kotoCoreLib Function

highlight default link kotoAsserts Macro
highlight default link kotoCapture Type
highlight default link kotoOperator Operator

highlight default link kotoBoolean Boolean
highlight default link kotoNumber Number
highlight default link kotoString String
