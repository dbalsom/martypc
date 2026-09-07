;   MartyPC
;   https://github.com/dbalsom/martypc
;
;   Copyright 2022-2026 Daniel Balsom
;
;   Permission is hereby granted, free of charge, to any person obtaining a
;   copy of this software and associated documentation files (the "Software"),
;   to deal in the Software without restriction, including without limitation
;   the rights to use, copy, modify, merge, publish, distribute, sublicense,
;   and/or sell copies of the Software, and to permit persons to whom the
;   Software is furnished to do so, subject to the following conditions:
;
;   The above copyright notice and this permission notice shall be included in
;   all copies or substantial portions of the Software.
;
;   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
;   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
;   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
;   AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
;   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
;   FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
;   DEALINGS IN THE SOFTWARE.
;
;   ---------------------------------------------------------------------------
;
;   mspeed.asm
;
;   Set MartyPC's emulation speed. The argument is an unsigned 16-bit value in
;   tenths of a percent: 1000 is 100.0% (a 1.0x speed multiplier).
;
;   Version 1.0.1
;     - Use the DOS-style, case-insensitive /S switch
;
;   Version 1.0.0
;     - Initial version

cpu 8086
bits 16
org 100h

section .text

start:
    push    cs
    pop     ds

    call    parse_command_line
    jc      invalid_argument
    mov     [requested_speed], ax

    call    marty_probe
    jc      service_unavailable

    cmp     byte [silent_mode], 0
    jne     check_service_available
    call    marty_print_version

check_service_available:
    test    byte [cs:marty_si_flags], MARTY_SERVICE_FLAG_AVAILABLE
    jz      service_unavailable

    mov     al, [cs:marty_si_flags]
    and     al, MARTY_SERVICE_FLAG_ENABLED
    mov     [service_was_enabled], al

    call    marty_enable_service
    jc      service_unavailable

    call    query_speed
    jc      service_rejected
    mov     [current_speed], cx

    mov     ah, FUNC_SPEED_CONTROL
    mov     al, SPEED_CONTROL_SET
    mov     cx, [requested_speed]
    call    marty_service
    jc      service_rejected

    call    query_speed
    jc      service_rejected
    mov     [final_speed], cx

    cmp     byte [service_was_enabled], 0
    jne     print_success
    call    marty_disable_service

print_success:
    cmp     byte [silent_mode], 0
    jne     success

    mov     dx, current_prefix
    mov     ax, [current_speed]
    call    print_speed_line

    mov     dx, requested_prefix
    mov     ax, [requested_speed]
    call    print_speed_line

    mov     dx, final_prefix
    mov     ax, [final_speed]
    call    print_speed_line

success:
    mov     ax, 4C00h
    int     21h

; Query the current speed-control values.
;
; Output:
;   CF clear: BX = minimum, CX = current, DX = maximum
;   CF set:   query rejected
query_speed:
    mov     ah, FUNC_SPEED_CONTROL
    mov     al, SPEED_CONTROL_QUERY
    call    marty_service
    ret

; Print a labeled fixed-point speed as a percentage.
;
; Input:
;   CS:DX = '$'-terminated label
;   AX    = speed in tenths of a percent
print_speed_line:
    push    ax
    mov     ah, 09h
    int     21h
    pop     ax

    xor     dx, dx
    mov     bx, 10
    div     bx                      ; AX = whole percent, DX = tenths
    mov     [speed_fraction], dl

    xor     dx, dx
    call    print_decimal_dword

    mov     dl, '.'
    mov     ah, 02h
    int     21h

    mov     dl, [speed_fraction]
    add     dl, '0'
    mov     ah, 02h
    int     21h

    mov     dx, percent_suffix
    mov     ah, 09h
    int     21h
    ret

invalid_argument:
    cmp     byte [silent_mode], 0
    jne     .exit
    mov     dx, usage_message
    mov     ah, 09h
    int     21h

.exit:
    mov     ax, 4C01h
    int     21h

service_unavailable:
    cmp     byte [silent_mode], 0
    jne     .exit
    mov     dx, unavailable_message
    mov     ah, 09h
    int     21h

.exit:
    mov     ax, 4C02h
    int     21h

service_rejected:
    cmp     byte [service_was_enabled], 0
    jne     .print_error
    call    marty_disable_service

.print_error:
    cmp     byte [silent_mode], 0
    jne     .exit
    mov     dx, rejected_message
    mov     ah, 09h
    int     21h

.exit:
    mov     ax, 4C03h
    int     21h

; Parse an optional leading /S flag and one unsigned decimal argument from the
; DOS command tail.
;
; Output:
;   CF clear: AX = value from 0 through 65535
;   CF set:   argument missing, malformed, or out of range
parse_command_line:
    xor     ch, ch
    mov     cl, [80h]
    mov     si, 81h

.skip_leading_space:
    jcxz    .invalid_short
    mov     dl, [si]
    cmp     dl, ' '
    je      .consume_leading_space
    cmp     dl, 09h
    jne     .first_digit

.consume_leading_space:
    inc     si
    dec     cx
    jmp     .skip_leading_space

.invalid_short:
    jmp     .invalid

.first_digit:
    cmp     dl, '/'
    jne     .require_digit
    cmp     cx, 2
    jb      .invalid
    mov     dl, [si + 1]
    cmp     dl, 'S'
    je      .silent_option
    cmp     dl, 's'
    je      .silent_option
    jmp     .invalid

.silent_option:
    mov     byte [silent_mode], 1
    add     si, 2
    sub     cx, 2
    jcxz    .invalid
    mov     dl, [si]
    cmp     dl, ' '
    je      .skip_leading_space
    cmp     dl, 09h
    je      .skip_leading_space
    jmp     .invalid

.require_digit:
    cmp     dl, '0'
    jb      .invalid
    cmp     dl, '9'
    ja      .invalid
    xor     ax, ax

.digit_loop:
    jcxz    .success
    mov     dl, [si]
    cmp     dl, '0'
    jb      .trailing_space
    cmp     dl, '9'
    ja      .trailing_space

    sub     dl, '0'
    xor     dh, dh
    mov     bp, dx
    mov     bx, 10
    mul     bx
    test    dx, dx
    jnz     .invalid
    add     ax, bp
    jc      .invalid

    inc     si
    dec     cx
    jmp     .digit_loop

.trailing_space:
    cmp     dl, ' '
    je      .consume_trailing_space
    cmp     dl, 09h
    jne     .invalid

.consume_trailing_space:
    inc     si
    dec     cx
    jcxz    .success
    mov     dl, [si]
    cmp     dl, ' '
    je      .consume_trailing_space
    cmp     dl, 09h
    je      .consume_trailing_space

.invalid:
    stc
    ret

.success:
    clc
    ret

%include "../common/marty.inc"

section .data

current_speed           dw 0
requested_speed         dw 0
final_speed             dw 0
speed_fraction          db 0
service_was_enabled     db 0
silent_mode             db 0
current_prefix          db 'Current emulation speed:   ', '$'
requested_prefix        db 'Requested emulation speed: ', '$'
final_prefix            db 'Applied emulation speed:   ', '$'
percent_suffix          db '%', 0Dh, 0Ah, '$'
usage_message           db 'Usage: MSPEED [/S] value', 0Dh, 0Ah
                        db '  value is tenths of a percent (1000 = 100.0%)', 0Dh, 0Ah, '$'
unavailable_message     db 'MartyPC service interrupt is unavailable.', 0Dh, 0Ah, '$'
rejected_message        db 'MartyPC rejected the speed request.', 0Dh, 0Ah, '$'
