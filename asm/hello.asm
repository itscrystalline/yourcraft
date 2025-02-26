

section .data
  a dq 0x01
  b dq 0x02
  msg db "hello", 10

section .text
  global _start

_start:
  mov rax, [a]
  mov rbx, [b]
  add rax, rbx

  ; mov rcx, rax
  ; add rcx, 48

  jmp .print

.print:
  mov rax, 1
  mov rdi, 1
  mov rsi, 51 
  mov rdx, 13
  syscall
  jmp .end

.end:
  mov rax, 60
  mov rdi, 0
  syscall

