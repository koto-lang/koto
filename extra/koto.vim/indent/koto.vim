if exists("b:did_indent")
  finish
endif
let b:did_indent = 1

" if exists('*KotoIndent')
"   finish
" endif

let s:cpo_save = &cpo
set cpo&vim

setlocal nolisp
setlocal autoindent
setlocal expandtab
setlocal indentexpr=KotoIndent()
setlocal nosmartindent

function! KotoIndent()
  let l:lnum = v:lnum - 1

  " No indent at start of file
  if l:lnum == 0
    return 0
  endif

  let l:indent = indent(l:lnum)
  let l:prev_line = getline(l:lnum)
  let l:line = getline(v:lnum)

  " Ending in || or =
  if l:prev_line =~# '\(|.*|\)\|=\s*\(#.*\)*$'
    let l:indent += &shiftwidth
  endif

  " Starting in for, if, else
  if l:prev_line =~# '^\s*\(for\s\|if\s\|else\).*$'
    let l:indent += &shiftwidth
  endif

  return l:indent
endfunc

let &cpo = s:cpo_save
unlet s:cpo_save
