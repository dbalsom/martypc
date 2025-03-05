;   MartyPC
;   https://github.com/dbalsom/martypc
;
;   Copyright 2022-2025 Daniel Balsom
;
;   Permission is hereby granted, free of charge, to any person obtaining a
;   copy of this software and associated documentation files (the “Software”),
;   to deal in the Software without restriction, including without limitation
;   the rights to use, copy, modify, merge, publish, distribute, sublicense,
;   and/or sell copies of the Software, and to permit persons to whom the
;   Software is furnished to do so, subject to the following conditions:
;
;   The above copyright notice and this permission notice shall be included in
;   all copies or substantial portions of the Software.
;    
;   THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
;   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
;   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
;   AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER   
;   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
;   FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
;   DEALINGS IN THE SOFTWARE.
;
;   ---------------------------------------------------------------------------
;
;   mquit.asm
;  
;   Version 0.1
;
;   This utility will quit the MartyPC, passing a command line value from 
;   0-255, intended to be used as a delay parameter.
;
;   Compile with nasm.  nasm mquit.asm -o mquit.com

cpu 8086
org 100h

BITS 16
ORG 100h

; This program parses a single numeric argument from the DOS command line
; (0-255) and places it into AL, then calls INT 0FCh with AH=3.
; We now use an 8-bit multiply (mul bl) for clarity.

        mov ax, cs
        mov ds, ax
        mov es, ax

        ; Move SI to the start of the command tail (first char)
        mov si, 81h
        call skip_spaces      ; Skip any leading spaces
        call parse_number     ; AL = 0..255

        mov ah, 3
        int 0FCh

        mov ax, 4C00h
        int 21h

;------------------------------------------
skip_spaces:
        ; This routine advances SI past any spaces.
        ; If the first char is not space, it returns immediately.

        mov cx, 128
.skip:
        mov al, [si]           ; Check the current char
        cmp al, ' '
        jne .done              ; If it's not space, we're done
        inc si                 ; Otherwise skip it
        loop .skip
.done:
        ret

;------------------------------------------
parse_number:
        ; AL holds the final 8-bit parsed value (0..255)
        ; We read from [si] until we see a carriage return or a non-digit.

        xor ax, ax            ; AX=0 => AL=0
        mov bl, 10            ; Use BL=10 for an 8-bit multiplication

.parse_loop:
        mov dl, [si]          ; Read the next character into DL
        cmp dl, 0Dh           ; Stop if carriage return
        je  .done

        cmp dl, '0'           ; Is it below '0'?
        jb  .done             ; Then stop
        cmp dl, '9'           ; Is it above '9'?
        ja  .done             ; Then stop

        ; Convert ASCII char to digit in DL
        sub dl, '0'

        ; Multiply AL by 10, accumulate digit in AL
        mul bl                ; 8-bit multiply: AX = AL * BL
        add al, dl            ; AL += digit

        inc si                ; Move to next char
        jmp .parse_loop

.done:
        ret
