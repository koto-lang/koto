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

  " Ending with || or =
  if l:prev_line =~# '^.*\(\(|.*|\)\|=\)\s*$'
    return l:indent + &shiftwidth
  endif

  " inline if/then statements don't need to be indented
  if l:prev_line =~# '\.*if\s.*then\s.*$'
    return l:indent
  endif

  " Ending with else, loop
  if l:prev_line =~# '^.*\(else\|loop\).*$'
    return l:indent + &shiftwidth
  endif

  " Containing for, until, while, if, else, match
  if l:prev_line =~# '^.*\(for\s\|until\s\|while\s\|if\s\|match\s\).*$'
    return l:indent + &shiftwidth
  endif

  " Containing try, catch, finally
  if l:prev_line =~# '^.*\(try\|catch\s\|finally\).*$'
    return l:indent + &shiftwidth
  endif

  return l:indent
endfunc

let &cpo = s:cpo_save
unlet s:cpo_save
