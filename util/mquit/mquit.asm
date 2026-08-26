;   MartyPC
;   https://github.com/dbalsom/martypc
;
;   Copyright 2022-2026 Daniel Balsom
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
;   This utility will quit MartyPC after the specified delay in seconds
;   from 0-255.
;
;   Version 1.1.0
;     - Add MartyPC probe
;     - Implement delay functionality using BIOS clock
;
;   Version 1.0.0
;     - Initial Release

cpu 8086
bits 16
org 100h

    mov     ax, cs
    mov     ds, ax
    mov     es, ax

    call    marty_probe               ; Are we running in MartyPC?
    jc      exit                      ; no - exit harmlessly

    mov     ah, FUNC_SERVICE_CTRL
    mov     al, SERVICE_CTRL_ENABLE
    mov     bx, SERVICE_CTRL_BX
    mov     cx, SERVICE_CTRL_CX
    call    marty_service            ; Enable emulator service
    jc      exit

    ; Move SI to the start of the command tail (first char)
    mov     si, 81h
    call    skip_spaces              ; Skip any leading spaces
    call    parse_number             ; AL = 0..255
    call    wait_seconds             ; Wait AL seconds

    mov     ah, FUNC_QUIT
    call    marty_service            ; Call emulator service to quit MartyPC

exit:
    mov     ax, 4C00h                ; Technically, we won't even get here, but just in case, quit cleanly
    int     21h

;------------------------------------------
skip_spaces:
    ; This routine advances SI past any spaces.
    ; If the first char is not space, it returns immediately.

    mov     cx, 128
.skip:
    mov     al, [si]                 ; Check the current char
    cmp     al, ' '
    jne     .done                    ; If it's not space, we're done
    inc     si                       ; Otherwise skip it
    loop    .skip
.done:
    ret

;------------------------------------------
wait_seconds:
    ; Wait for the number of seconds in AL using the BIOS time-of-day clock.
    ; INT 1Ah/AH=00h returns the tick count since midnight in CX:DX.
    ; Preserve AL so the delay value can still be passed to FUNC_QUIT.

    test    al, al
    jz      .done

    push    ax
    push    bx
    push    cx
    push    dx
    push    si
    push    di
    push    bp

    ; Convert seconds to timer ticks, rounding up. The BIOS clock advances at
    ; approximately 18.21 ticks per second, so ticks = ceil(seconds * 1821/100).
    xor     ah, ah
    mov     bx, 1821
    mul     bx
    add     ax, 99
    adc     dx, 0
    mov     bx, 100
    div     bx
    mov     bp, ax                   ; BP = ticks to wait

    mov     ah, 00h
    int     1Ah
    mov     si, cx                   ; SI:DI = starting tick count
    mov     di, dx

.poll:
    mov     ah, 00h
    int     1Ah

    ; If the current count is lower than the starting count, the BIOS clock
    ; rolled over at midnight while we were waiting.
    cmp     cx, si
    jb      .midnight_wrap
    ja      .no_wrap
    cmp     dx, di
    jb      .midnight_wrap

.no_wrap:
    mov     ax, dx
    mov     bx, cx
    sub     ax, di
    sbb     bx, si                   ; BX:AX = current - start
    jmp     .compare

.midnight_wrap:
    mov     ax, 00B0h
    mov     bx, 0018h                ; 0018:00B0h ticks per day
    sub     ax, di
    sbb     bx, si                   ; BX:AX = midnight - start
    add     ax, dx
    adc     bx, cx                   ; BX:AX += ticks since midnight

.compare:
    or      bx, bx
    jnz     .restore                 ; More than 65535 ticks have elapsed
    cmp     ax, bp
    jb      .poll

.restore:
    pop     bp
    pop     di
    pop     si
    pop     dx
    pop     cx
    pop     bx
    pop     ax

.done:
    ret

;------------------------------------------
parse_number:
    ; AL holds the final 8-bit parsed value (0..255)
    ; We read from [si] until we see a carriage return or a non-digit.

    xor     ax, ax                   ; AX=0 => AL=0
    mov     bl, 10                   ; Use BL=10 for an 8-bit multiplication

.parse_loop:
    mov     dl, [si]                 ; Read the next character into DL
    cmp     dl, 0Dh                  ; Stop if carriage return
    je      .done

    cmp     dl, '0'                  ; Is it below '0'?
    jb      .done                    ; Then stop
    cmp     dl, '9'                  ; Is it above '9'?
    ja      .done                    ; Then stop

    ; Convert ASCII char to digit in DL
    sub     dl, '0'

    ; Multiply AL by 10, accumulate digit in AL
    mul     bl                       ; 8-bit multiply: AX = AL * BL
    add     al, dl                   ; AL += digit

    inc     si                       ; Move to next char
    jmp     .parse_loop

.done:
    ret

%include "../common/marty.inc"
