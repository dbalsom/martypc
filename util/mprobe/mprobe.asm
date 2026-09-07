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

;   mprobe.asm
;
;   Detect whether the program is running under MartyPC.
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

    call    marty_probe              ; Probe for MartyPC
    jc      not_detected             ; MartyPC not found

    call    marty_print_version

    test    byte [cs:marty_si_flags], MARTY_SERVICE_FLAG_AVAILABLE
    jz      si_not_installed         ; service interrupt unavailable

    mov     dx, si_message
    mov     ah, 09h
    int     21h                      ; print 'Service interrupt vector: '

    mov     al, [cs:marty_si_vector]
    call    print_hex_byte           ; print vector #

    mov     dx, hex_suffix
    mov     ah, 09h
    int     21h                      ; print 'h'

    mov     ah, FUNC_SERVICE_CTRL
    mov     al, SERVICE_CTRL_QUERY
    mov     bx, SERVICE_CTRL_BX
    mov     cx, SERVICE_CTRL_CX
    call    marty_service            ; query service interrupt state

    cmp     al, SERVICE_CTRL_ENABLE
    je      si_enabled

    mov     dx, si_disabled_message
    mov     ah, 09h
    int     21h                      ; print 'Service interrupt disabled'
    jmp     print_api_version

si_enabled:
    mov     dx, si_enabled_message
    mov     ah, 09h
    int     21h                      ; print 'Service interrupt enabled'
    jmp     print_api_version

si_not_installed:
    mov     dx, si_not_installed_message
    mov     ah, 09h
    int     21h                      ; print 'Service interrupt not installed.'
    jmp     success

print_api_version:
    call    marty_print_api_version

success:
    mov     ax, 4C00h
    int     21h                      ; quit with exit code 0 == MartyPC installed

not_detected:
    mov     dx, not_detected_message
    mov     ah, 09h
    int     21h                      ; print 'MartyPC not detected'

    mov     ax, 4C01h
    int     21h                      ; quit with exit code 1 == MartyPC not installed

%include "../common/marty.inc"

section .data

not_detected_message      db 'MartyPC not detected', 0Dh, 0Ah, '$'
si_message                db 'Service interrupt vector: ', '$'
si_enabled_message        db 'Service interrupt enabled', 0Dh, 0Ah, '$'
si_disabled_message       db 'Service interrupt disabled', 0Dh, 0Ah, '$'
si_not_installed_message  db 'Service interrupt not installed.', 0Dh, 0Ah, '$'
hex_suffix                db 'h', 0Dh, 0Ah, '$'
